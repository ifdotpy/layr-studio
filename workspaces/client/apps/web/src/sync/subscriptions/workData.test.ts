import { expect, it } from "vitest";
import { WorkStatus } from "../../generated/api";
import type { SyncEntityPayload } from "../../generated/api";
import { createTaskAssignmentSelector } from "./workData";

it("uses server dependency policy even when the scoped cache omits a dependency", () => {
  const entities = [
    {
      collection: "workDetail",
      id: "child",
      value: {
        id: "child",
        rootId: "lead",
        title: "Child",
        created: 1,
        updated: 1,
        version: 1,
        status: WorkStatus.ready,
        dependencies: ["parent"],
        policy: {
          blockedBy: [],
          displayStatus: WorkStatus.ready,
          acceptAvailable: true,
          gateOverrideRequired: false,
        },
        results: [],
        decisions: [],
        releases: [],
      },
    },
  ] satisfies SyncEntityPayload[];
  const [task] = createTaskAssignmentSelector()(entities, "lead");
  expect(task?.policy.blockedBy).toEqual([]);
  expect(task?.policy.displayStatus).toBe("ready");
});
