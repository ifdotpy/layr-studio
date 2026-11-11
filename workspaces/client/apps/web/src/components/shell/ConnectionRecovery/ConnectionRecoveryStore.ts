import { makeAutoObservable } from "mobx";
import { RecoveryStatus } from "../../../generated/api.ts";
import { errorText } from "../../../api.ts";
import { flowValue } from "../../../flowValue.ts";
import type { ServerApi } from "../../../servers/net/api.ts";

export class ConnectionRecoveryStore {
  pending = false;
  message = "";
  details = "";
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true });
  }
  *check(post: ServerApi["post"], agentId: string) {
    if (this.pending) return;
    this.pending = true;
    this.message = this.details = "";
    try {
      const result = yield* flowValue(
        post(
          "/api/connection-recovery",
          { id: agentId },
          { timeoutMs: 100000 },
        ),
      );
      if (
        result.status === RecoveryStatus.reconciled &&
        ["completed", "failed", "interrupted"].includes(result.outcome || "")
      )
        this.message = "Connection checked.";
      else if (result.status === RecoveryStatus.superseded)
        this.message =
          "The conversation changed while the connection was checked. Review its current state.";
      else {
        this.details = result.error ? errorText(result.error) : "";
        this.message = result.error
          ? "Could not confirm the previous outcome. Review the history before continuing."
          : "Connection checked. The previous outcome remains unconfirmed. Review the history before continuing.";
      }
    } catch (error) {
      this.message = "Could not finish the connection check.";
      this.details = errorText(error);
    } finally {
      this.pending = false;
    }
  }
}
