import { makeAutoObservable, reaction } from "mobx";
import { requestId as resourceRequestId } from "../../../generated/observableResources.ts";
import { resourceData } from "../../../store/runtime.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { errorText, type PostResult } from "../../../api.ts";
import type { DeepReadonly } from "../../../deepReadonly.ts";
import type { Account } from "./Accounts.tsx";
import type { ServerServices } from "../../../servers/serverServices.ts";
import { copyText } from "../../../clipboard/clipboard.ts";

type Receipt = PostResult<"/api/accounts/login">;
type Options = {
  scope: string;
  accountKey?: string;
  email: string;
  label: string;
  onReady?: (account: Account) => unknown;
};

/** Account-dialog lifetime; persisted command identities survive lost replies and remounts. */
export class CodexAuthFlow {
  requestId: string;
  busy = "";
  error = "";
  copied = false;
  expectedEmail: string;
  #reported = "";
  readonly #services: Pick<ServerServices, "api" | "store" | "resources">;
  readonly #options: Options;
  readonly #stop;
  constructor(
    services: Pick<ServerServices, "api" | "store" | "resources">,
    options: Options,
  ) {
    this.#services = services;
    this.#options = options;
    this.expectedEmail = options.email;
    this.requestId = services.api.saved(this.storageKey, "");
    makeAutoObservable(this, {}, { autoBind: true });
    this.#stop = reaction(
      () =>
        ["ready", "duplicate"].includes(this.receipt?.status || "")
          ? this.account
          : undefined,
      (account) => {
        if (account && this.#reported !== this.requestId) {
          this.#reported = this.requestId;
          void Promise.resolve(options.onReady?.(account)).catch(this.fail);
        }
      },
      { fireImmediately: true },
    );
  }
  get storageKey() {
    return `account-sign-in:${this.#options.scope}${this.#options.accountKey ? `:${this.#options.accountKey}` : ""}`;
  }
  get receipt() {
    const value = (
      this.requestId
        ? resourceData(
            this.#services.store.authProgress(resourceRequestId(this.requestId))
              .current,
          )
        : undefined
    ) as DeepReadonly<Receipt> | undefined;
    return value?.requestId === this.requestId &&
      (!this.#options.accountKey ||
        value.accountKey === this.#options.accountKey)
      ? value
      : undefined;
  }
  get account() {
    return resourceData(this.#services.store.state().current)
      ?.accountsSummary.get("current")
      ?.accounts.find((a) => a.id === this.receipt?.resolvedAccountKey);
  }
  get url() {
    try {
      const value = new URL(this.receipt?.verificationUrl || "");
      return value.protocol === "https:" &&
        !value.username &&
        !value.password &&
        (value.hostname === "openai.com" ||
          value.hostname.endsWith(".openai.com"))
        ? value.href
        : null;
    } catch {
      return null;
    }
  }
  dispose() {
    this.#stop();
  }
  fail(failure: unknown) {
    this.error = errorText(failure);
  }
  remember(id: string) {
    this.#services.api.save(this.storageKey, id);
    this.requestId = id;
  }
  accept(result: Receipt) {
    this.remember(result.requestId);
    this.#services.resources.publish(
      { kind: "auth-progress", requestId: result.requestId },
      result,
    );
  }
  *run(
    name: "start" | "cancel" | "check" | "copy",
    emailOverride?: string,
  ): Generator<Promise<Receipt | void>, void, Receipt> {
    const receipt = this.receipt,
      target = this.#options.accountKey;
    if (
      this.busy ||
      (name !== "start" && (!receipt || (name === "check" && !target)))
    )
      return;
    if (emailOverride) this.expectedEmail = emailOverride;
    this.busy = name;
    this.error = "";
    const id =
      name !== "start" || receipt?.status === "uncertain"
        ? receipt!.requestId
        : !receipt && this.requestId
          ? this.requestId
          : randomUUID();
    const key = `${this.storageKey}:${id}:${name}`;
    try {
      if (name === "start") this.remember(id);
      if (name === "copy") {
        if (receipt?.userCode) {
          yield copyText(receipt.userCode);
          this.copied = true;
        }
        return;
      }
      const result = yield this.#services.api.post(
        name === "cancel"
          ? "/api/accounts/login/cancel"
          : "/api/accounts/login",
        {
          login_id: id,
          ...(name === "cancel"
            ? {}
            : {
                ...(target ? { account_key: target } : {}),
                ...(!target && (emailOverride || this.expectedEmail)
                  ? { email: emailOverride || this.expectedEmail }
                  : {}),
                ...(!target && this.#options.label
                  ? { label: this.#options.label }
                  : {}),
              }),
        },
        {
          timeoutMs: name === "cancel" ? 15000 : 30000,
          requestId: this.#services.api.stableRequestId(key),
        },
      );
      this.accept(name === "cancel" ? result : { ...result, requestId: id });
      this.#services.api.clearStableRequestId(key);
      if (name === "start") this.copied = false;
    } catch (failure) {
      this.fail(failure);
    } finally {
      this.busy = "";
    }
  }
}
