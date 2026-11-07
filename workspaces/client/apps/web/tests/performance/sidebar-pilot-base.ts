import type { Agent } from "../../src/types.ts";
import { chatIndicators } from "../../src/components/chat-status/chatStatusModel.ts";
import { pilotSnapshot } from "./sidebar-pilot-data.ts";

/** Selected only by the baseline source overlay; the fixture stays identical. */
export function createPilotModel(agents: Agent[], notify: () => void) {
  let data = pilotSnapshot(agents);
  let indicators = chatIndicators(data);
  return {
    view: () => ({ data, indicators }),
    apply(next: Agent[]) {
      data = pilotSnapshot(next);
      indicators = chatIndicators(data);
      notify();
    },
  };
}
