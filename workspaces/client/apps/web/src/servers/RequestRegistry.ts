import {
  flow,
  flowResult,
  getObserverTree,
  makeAutoObservable,
  observable,
  onBecomeUnobserved,
} from "mobx";
import { z } from "zod";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { ApiError, errorText, type post } from "../api.ts";
import type { createServerStore } from "../store/server.ts";
import { flowValue } from "../flowValue.ts";
import { serverMutationResponseIsUncertain } from "./mutationIdentity.ts";

export type RequestState =
  | "sending"
  | "unknown"
  | "confirmed"
  | "applied"
  | "applied-from-SSE"
  | "failed";
export type OperationKey = readonly [
  workspaceId: string,
  kind: string,
  target: string,
  fieldOrRole?: string,
];
export type ReceiptProof =
  | { kind: "applied" }
  | { kind: "failed"; errorText: string };
export type OperationContext = {
  api: { post: typeof post };
  store: () => ReturnType<typeof createServerStore>;
  notify: (message: string) => void;
};
/** One request with its typed body bound into send/watch; `body` is the persisted JSON. */
export type Intent = {
  key: OperationKey;
  path: string;
  body: unknown;
  label: string;
  retryable?: false;
  send: (
    context: OperationContext,
    entry: RequestEntry,
  ) => Promise<"applied" | void>;
  watch: (
    context: OperationContext,
    entry: RequestEntry,
    observe: (proof: ReceiptProof) => void,
  ) => () => void;
};
export type RequestOperation = {
  path: string;
  revive: (key: OperationKey, body: unknown) => Intent;
};
/** send returns "applied" only for an authoritative reply for this request id; otherwise
 * the entry is confirmed and waits for watch to observe an exact receipt for the id. */
/** Request bodies are plain JSON: persisted as JSON and resent as JSON. */
type Json =
  | string
  | number
  | boolean
  | null
  | Json[]
  | { [key: string]: Json | undefined };
/** The schema must be plain (no transform/default/preprocess): admission and revival must yield the
 * same body. Input = output is enforced by type; value-preserving plainness is the owner's duty. */
export function requestOperation<
  B extends { [key: string]: Json | undefined },
>(spec: {
  path: string;
  body: z.ZodType<B, B>;
  /** Names the request in unresolved-request notices. */
  label: (body: B) => string;
  send: (
    context: OperationContext,
    body: B,
    entry: RequestEntry,
  ) => Promise<"applied" | void>;
  watch?: (
    context: OperationContext,
    body: B,
    entry: RequestEntry,
    observe: (proof: ReceiptProof) => void,
  ) => () => void;
}) {
  const intent = (key: OperationKey, value: B): Intent => {
    // A retry and a reload must resend exactly the admitted wire body.
    const body = deepFreeze(spec.body.parse(structuredClone(value)));
    return {
      key: Object.freeze(structuredClone(key)),
      path: spec.path,
      body,
      label: spec.label(body),
      send: (context, entry) => spec.send(context, body, entry),
      watch: (context, entry, observe) =>
        spec.watch?.(context, body, entry, observe) ?? (() => {}),
    };
  };
  return {
    path: spec.path,
    intent,
    revive: (key: OperationKey, body: unknown) =>
      intent(key, spec.body.parse(body)),
  };
}
function deepFreeze<T>(value: T): T {
  if (value && typeof value === "object")
    Object.values(Object.freeze(value)).forEach(deepFreeze);
  return value;
}
/** Keeps an unresolved request of a no-longer-registered operation visible; it cannot resend. */
function opaqueIntent(key: OperationKey, path: string, body: unknown): Intent {
  return {
    key,
    path,
    body,
    label: `Request ${path}`,
    retryable: false,
    send: () =>
      Promise.reject(new Error("This request can no longer be resent.")),
    watch: () => () => {},
  };
}
const savedEntry = z.object({
  requestId: z.string().min(1),
  key: z.union([
    z.tuple([z.string(), z.string(), z.string()]),
    z.tuple([z.string(), z.string(), z.string(), z.string()]),
  ]),
  path: z.string(),
  body: z.unknown(),
  state: z.enum(["sending", "unknown", "confirmed"]),
});
const unknownText = "The result is unknown. Retry the saved request.";
const prefix = "studio-server-write:request:";
export class RequestEntry {
  readonly requestId;
  readonly key;
  readonly path;
  readonly body;
  readonly intent;
  state: RequestState = "sending";
  errorText = "";
  dismissed = false;
  completion: Promise<void> = Promise.resolve();
  constructor(intent: Intent, id: string) {
    this.requestId = id;
    this.key = intent.key;
    this.path = intent.path;
    this.body = intent.body;
    this.intent = intent;
    makeAutoObservable(this, {
      key: false,
      body: false,
      intent: false,
      completion: false,
    });
  }
  get terminal() {
    return ["applied", "applied-from-SSE", "failed"].includes(this.state);
  }
  get view() {
    return {
      state: this.state,
      busy: this.state === "sending",
      canRetry: this.state === "unknown" && this.intent.retryable !== false,
      unresolved: !this.terminal,
      errorText: this.dismissed ? "" : this.errorText,
    };
  }
}
export class RequestRegistry {
  readonly entries = observable.map<string, RequestEntry>({}, { deep: false });
  readonly latestIds = new Map<string, string>();
  readonly stops = new Map<string, () => void>();
  readonly storage;
  readonly context;
  disposed = false;
  constructor(
    storage: Storage,
    context: OperationContext,
    operations: readonly RequestOperation[],
  ) {
    this.storage = storage;
    this.context = context;
    makeAutoObservable(
      this,
      { storage: false, context: false, stops: false },
      { autoBind: true },
    );
    const keys = Array.from({ length: storage.length }, (_, i) =>
      storage.key(i),
    );
    for (const key of keys) {
      if (!key?.startsWith(prefix)) continue;
      const saved = savedEntry.parse(
        JSON.parse(storage.getItem(key) || "null"),
      );
      const operation = operations.find(({ path }) => path === saved.path);
      const entry = new RequestEntry(
        operation?.revive(saved.key, saved.body) ??
          opaqueIntent(saved.key, saved.path, saved.body),
        saved.requestId,
      );
      entry.state = saved.state === "sending" ? "unknown" : saved.state;
      if (entry.state === "unknown") entry.errorText = unknownText;
      this.attach(entry);
    }
  }
  attach(entry: RequestEntry) {
    this.entries.set(entry.requestId, entry);
    this.latestIds.set(JSON.stringify(entry.key), entry.requestId);
    const stops = [
      onBecomeUnobserved(entry, "view", () =>
        queueMicrotask(() => this.release(entry)),
      ),
    ];
    this.stops.set(entry.requestId, () => stops.forEach((stop) => stop()));
    const stopProof = entry.intent.watch(this.context, entry, (proof) =>
      this.observe(entry.requestId, proof),
    );
    if (this.entries.has(entry.requestId)) stops.push(stopProof);
    else stopProof();
  }
  start(intent: Intent): RequestEntry {
    if (this.disposed) throw new Error("The server request scope is closed.");
    const entry = new RequestEntry(intent, randomUUID());
    this.persist(entry); // Write before dispatch; a failure must prevent the send.
    this.attach(entry);
    if (entry.state === "sending")
      entry.completion = flowResult(this.send(entry, false));
    return entry;
  }
  // Only owner-boundary conversion may admit an old unresolved identity; it never dispatches.
  restore(intent: Intent, id: string) {
    const existing = this.get(id);
    if (existing) return existing;
    const entry = new RequestEntry(intent, id);
    entry.state = "unknown";
    entry.errorText = unknownText;
    this.persist(entry);
    this.attach(entry);
    return entry;
  }
  get(id: string) {
    return this.entries.get(id);
  }
  latest(key: OperationKey) {
    return this.get(this.latestIds.get(JSON.stringify(key)) || "");
  }
  forTarget(workspace: string, target: string) {
    return [...this.entries.values()].filter(
      (entry) => entry.key[0] === workspace && entry.key[2] === target,
    );
  }
  retry = flow(function* (this: RequestRegistry, id: string) {
    const entry = this.get(id);
    if (!entry || this.disposed) return;
    if (entry.state === "sending") return yield* flowValue(entry.completion);
    if (entry.state !== "unknown" || entry.intent.retryable === false) return;
    this.persist(entry);
    entry.state = "sending";
    entry.dismissed = false;
    entry.completion = flowResult(this.send(entry, true));
    yield* flowValue(entry.completion);
  });
  *send(entry: RequestEntry, uncertain: boolean) {
    try {
      const outcome = yield* flowValue(entry.intent.send(this.context, entry));
      if (this.disposed || entry.terminal) return;
      entry.state = outcome === "applied" ? outcome : "confirmed";
      entry.errorText = "";
    } catch (error) {
      if (this.disposed || entry.terminal) return;
      const rejected =
        !uncertain &&
        error instanceof ApiError &&
        error.status >= 400 &&
        error.status < 500 &&
        !serverMutationResponseIsUncertain(error.status, error.response);
      entry.state = rejected ? "failed" : "unknown";
      entry.errorText = rejected ? errorText(error) : unknownText;
    }
    if (!this.disposed && this.get(entry.requestId) === entry) {
      this.persist(entry);
      this.release(entry);
    }
  }
  observe(id: string, proof: ReceiptProof) {
    const entry = this.get(id);
    if (!entry || this.disposed || entry.terminal) return;
    entry.state = proof.kind === "applied" ? "applied-from-SSE" : "failed";
    entry.errorText = proof.kind === "failed" ? proof.errorText : "";
    this.persist(entry);
    this.release(entry);
  }
  dismiss(id: string) {
    const entry = this.get(id);
    if (entry) entry.dismissed = true;
  }
  persist(entry: RequestEntry) {
    const { requestId, key, path, body, state } = entry;
    if (entry.terminal) this.storage.removeItem(prefix + requestId);
    else
      this.storage.setItem(
        prefix + requestId,
        JSON.stringify({ requestId, key, path, body, state }),
      );
  }
  release(entry: RequestEntry) {
    // Defer observer-loss cleanup beyond rendering; confirmed work still awaits SSE.
    if (this.get(entry.requestId) !== entry) return;
    if (!entry.terminal || !!getObserverTree(entry, "view").observers?.length)
      return;
    this.stops.get(entry.requestId)?.();
    this.stops.delete(entry.requestId);
    this.entries.delete(entry.requestId);
    const key = JSON.stringify(entry.key);
    if (this.latestIds.get(key) === entry.requestId) this.latestIds.delete(key);
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    for (const entry of this.entries.values()) {
      if (entry.state === "sending") entry.state = "unknown";
      this.stops.get(entry.requestId)?.();
    }
    this.stops.clear();
    this.entries.clear();
    this.latestIds.clear();
  }
}
