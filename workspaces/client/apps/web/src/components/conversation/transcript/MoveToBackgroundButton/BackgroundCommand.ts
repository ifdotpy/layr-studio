import { makeAutoObservable } from "mobx";
import { flowValue } from "../../../../flowValue.ts";
import type { ServerApi } from "../../../../api.ts";
import {
  checkCommandBackground,
  requestCommandBackground,
  type BackgroundOutcome,
} from "../support/commandBackground.ts";
export class BackgroundCommand {
  busy = false;
  result: BackgroundOutcome | undefined;
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true, deep: false });
  }
  get done() {
    return this.result?.kind === "backgrounded";
  }
  get message() {
    return this.result && "message" in this.result
      ? this.result.message
      : this.result?.kind === "notRunning"
        ? "The command already ended or is in the background."
        : undefined;
  }
  *run(
    api: ServerApi,
    agentId: string,
    turnId: string | undefined,
    itemId: string,
  ) {
    if (this.busy || this.done) return;
    this.busy = true;
    try {
      this.result = yield* flowValue(
        this.result?.kind === "uncertain"
          ? checkCommandBackground(api, this.result.request)
          : requestCommandBackground(api, agentId, turnId, itemId),
      );
    } finally {
      this.busy = false;
    }
  }
}
