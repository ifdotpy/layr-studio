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
import { ApiError, errorText, type PostBody } from "../api.ts";
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
// P1 admits only the generated rename contract; widen with the next migrated operation.
export type Intent = {
  key: OperationKey;
  path: "/api/rename";
  body: Readonly<Omit<PostBody<"/api/rename">, "request_id" | "requestId">>;
};
export type ReceiptProof =
  | { kind: "applied" }
  | { kind: "failed"; errorText: string };
const savedEntry = z.object({
  requestId: z.string().min(1),
  key: z.union([
    z.tuple([z.string(), z.string(), z.string()]),
    z.tuple([z.string(), z.string(), z.string(), z.string()]),
  ]),
  path: z.literal("/api/rename"),
  body: z
    .object({ id: z.string(), name: z.string().nullable().optional() })
    .strict(),
  state: z.enum(["sending", "unknown", "confirmed"]),
});
const unknownText = "The result is unknown. Retry the saved request.";
const prefix = "studio-server-write:request:";
export class RequestEntry {
  readonly requestId;
  readonly key;
  readonly path;
  readonly body;
  state: RequestState = "sending";
  errorText = "";
  dismissed = false;
  completion: Promise<void> = Promise.resolve();
  constructor(intent: Intent, id: string) {
    this.requestId = id;
    this.key = Object.freeze(structuredClone(intent.key));
    this.path = intent.path;
    this.body = Object.freeze({ ...intent.body });
    makeAutoObservable(this, { key: false, body: false, completion: false });
  }
  get terminal() {
    return ["applied", "applied-from-SSE", "failed"].includes(this.state);
  }
  get view() {
    return {
      state: this.state,
      busy: this.state === "sending",
      canRetry: this.state === "unknown",
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
  readonly dispatch;
  readonly watch;
  disposed = false;
  constructor(
    storage: Storage,
    dispatch: (entry: RequestEntry) => Promise<"applied" | void>,
    watch: (
      entry: RequestEntry,
      observe: (proof: ReceiptProof) => void,
    ) => () => void,
  ) {
    this.storage = storage;
    this.dispatch = dispatch;
    this.watch = watch;
    makeAutoObservable(
      this,
      { storage: false, dispatch: false, watch: false, stops: false },
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
      const entry = new RequestEntry(saved, saved.requestId);
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
    const stopProof = this.watch(entry, (proof) =>
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
    if (entry.state !== "unknown") return;
    this.persist(entry);
    entry.state = "sending";
    entry.dismissed = false;
    entry.completion = flowResult(this.send(entry, true));
    yield* flowValue(entry.completion);
  });
  *send(entry: RequestEntry, uncertain: boolean) {
    try {
      const outcome = yield* flowValue(this.dispatch(entry));
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
