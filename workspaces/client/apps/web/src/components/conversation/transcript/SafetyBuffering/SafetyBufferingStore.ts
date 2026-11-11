import { makeAutoObservable } from "mobx";
import { errorText, type ServerApi } from "../../../../api.ts";
import type { Agent } from "../../../../types.ts";
import type { AgentDetailEntityDto } from "../../../../generated/observableResources.ts";
import { NativeSafetyStage as GeneratedApiNativeSafetyStage } from "../../../../generated/api.ts";
import { derived } from "../../support/derived.ts";
export class SafetyBufferingStore {
  confirm = false;
  pending = false;
  error = "";
  dismissed = "";
  state = derived((agent: Agent, detail: Agent | AgentDetailEntityDto) => {
    const b = detail.nativeSafetyBuffering;
    const stored = detail.nativeSafetyRetry;
    const retry =
      stored?.epoch === agent.epoch &&
      stored?.accountKey === (agent.accountKey || "default")
        ? stored
        : null;
    const bufferingTurnId = b?.turnId || "";
    const retryTurnId = retry?.turnId || "";
    const turnId = bufferingTurnId || retryTurnId;
    const active =
      !!retry?.stage &&
      !["running", "failed", "cancelled"].includes(retry.stage);
    const visible =
      b?.showBufferingUi &&
      !b.responseStarted &&
      !!bufferingTurnId &&
      bufferingTurnId === agent.turnId &&
      agent.inFlight &&
      !b.dismissed &&
      this.dismissed !== bufferingTurnId;
    const failed =
      retry?.stage === GeneratedApiNativeSafetyStage.failed &&
      (retry.turnId === agent.turnId || !agent.turnId);

    return { b, retry, turnId, active, visible, failed };
  });
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true, deep: false });
  }
  setConfirm(value: boolean) {
    this.confirm = value;
  }
  *choose(
    api: ServerApi,
    agentId: string,
    turnId: string,
    safety: "wait" | "retry" | "cancel",
  ) {
    if (this.pending || !turnId) return;
    this.pending = true;
    this.error = "";
    try {
      yield api.post(
        "/api/action",
        { id: agentId, action: { safety, turnId } },
        { timeoutMs: 15000 },
      );
      this.confirm = false;
      if (safety === "wait") this.dismissed = turnId;
    } catch (cause) {
      this.error = errorText(cause);
    } finally {
      this.pending = false;
    }
  }
}
