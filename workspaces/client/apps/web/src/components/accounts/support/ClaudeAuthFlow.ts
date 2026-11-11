import { reached } from "../../../clock.ts";
import { makeAutoObservable, reaction } from "mobx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { requestId as resourceRequestId } from "../../../generated/observableResources.ts";
import { resourceData } from "../../../store/runtime.ts";
import { errorText, type PostResult } from "../../../api.ts";
import type { ServerServices } from "../../../servers/serverServices.ts";
import { copyText } from "../../../clipboard/clipboard.ts";
import {
  codeSubmissionNotApplied,
  codeSubmissionResolved,
  codeVerificationPending,
} from "./claudeCodeSubmission.ts";

type Receipt = PostResult<"/api/accounts/claude/login">;
type Attempt = {
  requestId: string;
  startRequestId: string;
  codeRequestId?: string | undefined;
  cancelRequestId?: string | undefined;
  label: string;
  email: string;
  createdAt: number;
  unknownSince?: number | undefined;
  codeSubmissionUncertain?: boolean | undefined;
};
type Options = {
  scope: string;
  accountKey?: string;
  label: string;
  email: string;
  onReady?: () => unknown;
};
const UNKNOWN_REQUEST_GRACE_MS = 5000;
const unknownRequest = (failure: unknown) =>
  (failure &&
    typeof failure === "object" &&
    "status" in failure &&
    failure.status === 404) ||
  /unknown claude sign-in request/i.test(errorText(failure));

/** One Claude subscription sign-in, either a new profile or an exact saved account. */
export class ClaudeAuthFlow {
  flow: Attempt | null;
  code = "";
  copied = false;
  error = "";
  busy = "";
  expectedEmail: string;
  private reported = "";
  private stop: (() => void) | undefined;
  readonly #api: ServerServices["api"];
  readonly #store: ServerServices["store"];
  readonly #resources: Pick<
    ServerServices["resources"],
    "publish" | "retryNow"
  >;
  readonly #options: Options;
  constructor(
    api: ServerServices["api"],
    store: ServerServices["store"],
    resources: Pick<ServerServices["resources"], "publish" | "retryNow">,
    options: Options,
  ) {
    this.#api = api;
    this.#store = store;
    this.#resources = resources;
    this.#options = options;
    this.expectedEmail = options.email;
    const id = api.saved<string>(this.storageKey, "");
    this.flow =
      options.accountKey && id
        ? {
            requestId: id,
            startRequestId: "",
            cancelRequestId: api.saved<string>(
              `${this.storageKey}:${id}:cancel-request-id`,
              "",
            ),
            label: options.label,
            email: options.email,
            createdAt: 0,
            codeSubmissionUncertain: api.saved(
              `${this.storageKey}:code-submission-uncertain`,
              false,
            ),
          }
        : id
          ? api.saved<Attempt | null>(`${this.storageKey}:${id}`, null)
          : null;
    makeAutoObservable(this, {}, { autoBind: true });
    this.stop = reaction(
      () => [this.observed, this.receipt] as const,
      ([observed, receipt]) => {
        if (observed?.status === "error") {
          if (
            !this.#options.accountKey &&
            unknownRequest(observed.error) &&
            this.flow &&
            !this.flow.unknownSince
          ) {
            this.persist({ ...this.flow, unknownSince: Date.now() });
          } else if (
            !unknownRequest(observed.error) ||
            this.#options.accountKey
          )
            this.error = errorText(observed.error);
        }
        if (
          receipt &&
          this.flow &&
          codeSubmissionResolved(receipt) &&
          this.codeSubmissionUncertain
        )
          this.persist({
            ...this.flow,
            codeSubmissionUncertain: false,
            codeRequestId: undefined,
          });
        if (this.terminal) this.clearSaved();
        if (
          receipt?.status === "ready" &&
          this.reported !== receipt.requestId
        ) {
          this.reported = receipt.requestId;
          void Promise.resolve(this.#options.onReady?.()).catch(this.fail);
        }
      },
      { fireImmediately: true },
    );
  }
  get storageKey() {
    return this.#options.accountKey
      ? `claude-sign-in:${this.#options.scope}:${this.#options.accountKey}`
      : `claude-add-sign-in:${this.#options.scope}`;
  }
  get requestId() {
    return this.flow?.requestId || "";
  }
  get observed() {
    return this.requestId
      ? this.#store.authProgress(resourceRequestId(this.requestId)).current
      : undefined;
  }
  get receipt() {
    const observed = this.observed;
    const value = observed ? resourceData(observed) : undefined;
    return value?.requestId === this.requestId &&
      (!this.#options.accountKey ||
        value.accountKey === this.#options.accountKey) &&
      value.status !== "duplicate" &&
      value.status !== "uncertain"
      ? value
      : null;
  }
  get terminal() {
    return ["ready", "error", "cancelled"].includes(this.receipt?.status || "");
  }
  get pending() {
    return !!this.requestId && !this.terminal;
  }
  get codeSubmissionUncertain() {
    return this.flow?.codeSubmissionUncertain === true;
  }
  get verificationPending() {
    return codeVerificationPending(this.receipt);
  }
  get canSubmitCode() {
    return (
      this.receipt?.status === "pending" &&
      !this.verificationPending &&
      !this.codeSubmissionUncertain
    );
  }
  get statusMessage() {
    if (!this.pending) return "";
    if (this.codeSubmissionUncertain)
      return "Studio could not confirm whether Claude received the authorization response. Check sign-in status or cancel this request; do not submit it again.";
    if (this.verificationPending)
      return "The full authorization response was submitted. Waiting for Claude to verify it; do not submit it again.";
    return this.#options.accountKey
      ? this.receipt?.status === "pending"
        ? "Complete sign-in in your browser."
        : "Waiting for the sign-in request."
      : this.receipt
        ? "Waiting for sign-in…"
        : "";
  }
  get unknownTooLong() {
    return (
      !!this.flow?.unknownSince &&
      !this.receipt &&
      reached(this.flow.unknownSince + UNKNOWN_REQUEST_GRACE_MS)
    );
  }
  get displayLabel() {
    return this.flow?.label || this.#options.label;
  }
  get displayEmail() {
    return this.flow?.email || this.expectedEmail;
  }
  get wrong() {
    return !!this.receipt?.error?.includes("different Claude account");
  }
  get displayError() {
    const receipt = this.receipt;
    if (!this.#options.accountKey && this.wrong)
      return `You signed in as ${receipt?.email || "another account"}. This account expects ${this.#options.email || "a different account"}. Studio did not save the sign-in.`;
    if (!this.#options.accountKey && receipt?.error?.includes("expired"))
      return "The sign-in expired. Start a new sign-in to get a new link.";
    return this.error || receipt?.error || "";
  }
  get url() {
    try {
      const url = new URL(this.receipt?.verificationUrl || "");
      return url.protocol === "https:" &&
        !url.username &&
        !url.password &&
        !url.port &&
        (this.#options.accountKey
          ? ["claude.ai", "claude.com"].includes(url.hostname) &&
            ["/oauth/authorize", "/cai/oauth/authorize"].includes(url.pathname)
          : url.hostname === "claude.com" &&
            url.pathname === "/cai/oauth/authorize")
        ? url.href
        : null;
    } catch {
      return null;
    }
  }
  get startLabel() {
    if (!this.requestId || (this.terminal && !this.#options.accountKey))
      return this.receipt ? "Start again" : "Start sign-in";
    return !this.receipt && !this.codeSubmissionUncertain
      ? "Retry sign-in"
      : "";
  }
  setCode(value: string) {
    if (!this.busy && !this.codeSubmissionUncertain) {
      this.code = value;
      this.error = "";
    }
  }
  persist(next: Attempt) {
    this.flow = next;
    this.#api.save(this.storageKey, next.requestId);
    if (this.#options.accountKey) {
      this.#api.save(
        `${this.storageKey}:${next.requestId}:cancel-request-id`,
        next.cancelRequestId || "",
      );
      this.#api.save(
        `${this.storageKey}:code-submission-uncertain`,
        !!next.codeSubmissionUncertain,
      );
    } else this.#api.save(`${this.storageKey}:${next.requestId}`, next);
  }
  clearSaved() {
    if (!this.#options.accountKey && this.flow) {
      this.#api.save(`${this.storageKey}:${this.requestId}`, null);
      if (this.#api.saved<string>(this.storageKey, "") === this.requestId)
        this.#api.save(this.storageKey, null);
    }
  }
  dispose() {
    this.stop?.();
    this.stop = undefined;
  }
  check() {
    this.#resources.retryNow();
  }
  fail(failure: unknown) {
    this.error = errorText(failure);
  }
  *copy(): Generator<Promise<void>, void, void> {
    try {
      if (this.url) {
        yield copyText(this.url);
        this.copied = true;
      }
    } catch (failure) {
      this.fail(failure);
    }
  }
  *start(
    email = this.expectedEmail,
    forceNew = false,
  ): Generator<Promise<Receipt>, void, Receipt> {
    if (this.busy) return;
    this.busy = "start";
    this.error = "";
    const previous = this.flow;
    const next =
      !forceNew && previous && !this.terminal
        ? { ...previous, unknownSince: undefined }
        : {
            requestId: randomUUID(),
            startRequestId: randomUUID(),
            label: this.#options.label.trim() || "Claude Code",
            email: email.trim(),
            createdAt: Date.now(),
          };
    try {
      if (next.requestId !== previous?.requestId) this.clearSaved();
      this.persist(next);
      const mutationKey = `${this.storageKey}:${next.requestId}:start`;
      const result = this.#options.accountKey
        ? yield this.#api.post(
            "/api/accounts/claude/login",
            { login_id: next.requestId, account_key: this.#options.accountKey },
            {
              timeoutMs: 30000,
              requestId: this.#api.stableRequestId(mutationKey),
            },
          )
        : yield this.#api.post(
            "/api/accounts/claude/add",
            {
              login_id: next.requestId,
              label: next.label,
              ...(next.email ? { email: next.email } : {}),
            },
            { requestId: next.startRequestId },
          );
      this.accept(result);
      this.#api.clearStableRequestId(mutationKey);
      this.expectedEmail = next.email;
    } catch (failure) {
      this.error = errorText(failure);
    } finally {
      this.busy = "";
    }
  }
  accept(result: Receipt) {
    if (
      result.requestId !== this.requestId ||
      (this.#options.accountKey &&
        result.accountKey !== this.#options.accountKey)
    )
      throw new Error(
        "The sign-in response belongs to another account or request.",
      );
    this.#resources.publish(
      { kind: "auth-progress", requestId: result.requestId },
      result,
    );
  }
  *run(action: "code" | "cancel"): Generator<Promise<Receipt>, void, Receipt> {
    if (this.busy || !this.flow || (action === "code" && !this.canSubmitCode))
      return;
    this.busy = action;
    this.error = "";
    const field = action === "code" ? "codeRequestId" : "cancelRequestId";
    const mutationId = this.flow[field] || randomUUID();
    const next = {
      ...this.flow,
      [field]: mutationId,
      ...(action === "code" ? { codeSubmissionUncertain: true } : {}),
    };
    try {
      this.persist(next);
      const result = yield this.#api.post(
        action === "code"
          ? "/api/accounts/claude/login/code"
          : "/api/accounts/claude/login/cancel",
        {
          login_id: next.requestId,
          ...(action === "code" ? { code: this.code.trim() } : {}),
        },
        { timeoutMs: 30000, requestId: mutationId },
      );
      if (action === "code") this.code = "";
      this.persist({
        ...next,
        [field]: undefined,
        codeSubmissionUncertain:
          action === "code"
            ? !codeSubmissionResolved(result)
            : next.codeSubmissionUncertain,
      });
      this.accept(result);
    } catch (failure) {
      if (action === "code") {
        const rejected = codeSubmissionNotApplied(failure);
        this.persist({
          ...next,
          codeSubmissionUncertain: !rejected,
          codeRequestId: rejected ? undefined : next.codeRequestId,
        });
        if (!rejected) this.code = "";
      } else if (!this.#options.accountKey && unknownRequest(failure)) {
        this.clearSaved();
        this.flow = null;
        this.error = "";
        return;
      }
      this.error = errorText(failure);
    } finally {
      this.busy = "";
    }
  }
}
