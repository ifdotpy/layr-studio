import type { Agent, Snapshot } from "../../src/types.ts";
import type { ChatIndicator } from "../../src/status/operationalStatus.ts";
import { pilotSnapshot } from "./sidebar-pilot-data.ts";

// The benchmark injects the exact baseline implementation at this typed boundary.
// This declaration has no production compatibility export or runtime fallback.
declare const baselineChatIndicators: (
  data: Snapshot,
) => Map<string, ChatIndicator>;

/** Selected only by the baseline source overlay; the fixture stays identical. */
export function createPilotModel(agents: Agent[], notify: () => void) {
  let data = pilotSnapshot(agents);
  let indicators = baselineChatIndicators(data);
  return {
    view: () => ({ data, indicators }),
    apply(next: Agent[]) {
      data = pilotSnapshot(next);
      indicators = baselineChatIndicators(data);
      notify();
    },
  };
}
