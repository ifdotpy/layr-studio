import { observer } from "mobx-react-lite";
import type {
  BackgroundTaskLabel,
  BackgroundTaskKindLabel,
} from "../../../../generated/api.ts";
import {
  activeTask,
  type DisplayBackgroundTask,
} from "../support/backgroundTaskModel.ts";
import { probe } from "../../../../perf/backgroundTaskRenderProbe.ts";
export type TaskProps = { task: DisplayBackgroundTask };
export const labels: Record<BackgroundTaskLabel, string> = {
  running: "Running",
  starting: "Starting",
  pending: "Pending",
  stopping: "Stopping",
  approval: "Needs approval",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
  interrupted: "Interrupted",
  lost: "Outcome unknown",
  waitingForExit: "Waiting for exit",
  unknown: "Unknown",
};
export const kindLabels: Record<BackgroundTaskKindLabel, string> = {
  monitor: "Monitor",
  command: "Command",
  backgroundCommand: "Background command",
  tool: "Tool",
};
export const duration = (seconds: number): string =>
  seconds < 60
    ? `${Math.floor(seconds)}s`
    : seconds < 3600
      ? `${Math.floor(seconds / 60)}m ${Math.floor(seconds % 60)}s`
      : `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
export function elapsed(task: DisplayBackgroundTask, now: number) {
  if (task.durationMs != null) return duration(task.durationMs / 1000);
  if (!activeTask(task) && !task.finished) return "";
  return duration(Math.max(0, (task.finished || now) - task.created));
}
export const TaskStatus = observer(function TaskStatus({ task }: TaskProps) {
  probe("background-task-status", task.id);
  return <span className="task-status">{labels[task.display.label]}</span>;
});
