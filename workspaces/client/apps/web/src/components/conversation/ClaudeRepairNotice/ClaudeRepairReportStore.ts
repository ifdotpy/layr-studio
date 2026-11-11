import { whileObserved } from "../../../observed.ts";
import { makeAutoObservable, observableRef } from "mobx";
import type { Agent } from "../../../types.ts";
import type { ServerApi } from "../../../api.ts";
import type { ClaudeRepairReport } from "./claudeRepairModel.ts";

export class ClaudeRepairReportStore {
  result?: { scope: string; report?: ClaudeRepairReport; error?: string };
  attempt = 0;
  readonly api;
  readonly agent;
  constructor(
    api: Pick<ServerApi, "get">,
    agent: () => Agent | undefined,
    observe = true,
  ) {
    this.api = api;
    this.agent = agent;
    makeAutoObservable(
      this,
      { api: false, agent: false, result: observableRef },
      { autoBind: true },
    );
    if (observe)
      whileObserved(this, "result", () => {
        void this.refresh();
        return this.dispose;
      });
  }
  get current() {
    return this.result?.scope === repairScope(this.agent())
      ? this.result
      : undefined;
  }
  *refresh() {
    const agent = this.agent();
    const scope = repairScope(agent);
    const attempt = ++this.attempt;
    if (!agent?.threadId) return;
    try {
      const report: ClaudeRepairReport = yield this.api.get(
        "/api/claude/repair-report",
        { query: { id: agent.id } },
      );
      if (attempt === this.attempt) this.result = { scope, report };
    } catch {
      if (attempt === this.attempt)
        this.result = {
          scope,
          error: "The saved repair report is unavailable.",
        };
    }
  }
  dispose() {
    this.attempt++;
  }
}

export function repairScope(agent?: Agent) {
  const { id, threadId, epoch, accountKey } = agent || {};
  return JSON.stringify([id, threadId, epoch, accountKey]);
}
