import { BackgroundTaskKind } from "../generated/api";
import type { components } from "../generated/api";
import type { DeepReadonly } from "../deepReadonly";

export type RendererTaskKind = components["schemas"]["BackgroundTaskKind"];
export type DisplayLegacyTask = DeepReadonly<
  components["schemas"]["TaskRecordView"]
>;
export type DisplayProcessTask = DeepReadonly<
  components["schemas"]["ProcessView"]
>;
export type DisplayBackgroundTask = DeepReadonly<
  components["schemas"]["BackgroundTaskView"]["value"]
>;

export function processIdentity(
  task: DisplayProcessTask,
): components["schemas"]["ProcessIdentity"] {
  return {
    key: task.key,
    generation: task.generation,
    agent: task.agent,
    accountKey: task.accountKey,
    epoch: task.epoch,
  };
}

export const activeTask = (task: Pick<DisplayBackgroundTask, "display">) =>
  task.display.active;

export const isCommandTask = (task: Pick<DisplayLegacyTask, "display">) =>
  task.display.kind === BackgroundTaskKind.command;
