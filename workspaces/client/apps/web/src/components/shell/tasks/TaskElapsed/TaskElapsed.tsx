import type { TaskProps } from "../TaskStatus/TaskStatus.tsx";
import { Clock } from "lucide-react";
import { observer } from "mobx-react-lite";
import { now } from "../../../../clock.ts";
import {
  activeTask,
  type DisplayBackgroundTask,
} from "../support/backgroundTaskModel.ts";
import { elapsed } from "../TaskStatus/TaskStatus.tsx";
import { probe } from "../../../../perf/backgroundTaskRenderProbe.ts";
/** Mounted only with the open activity panel; no list or detail observes the clock. */
function useElapsed(task: DisplayBackgroundTask) {
  const ticking = activeTask(task) && !task.finished && task.durationMs == null;
  probe("background-task-elapsed", task.id);
  return elapsed(task, ticking ? now() / 1000 : Date.now() / 1000);
}
export const TaskElapsed = observer(function TaskElapsed({ task }: TaskProps) {
  return useElapsed(task);
});
export const TaskElapsedFact = observer(function TaskElapsedFact({
  task,
}: TaskProps) {
  const age = useElapsed(task);
  return age ? (
    <span>
      <Clock size={12} />
      {age}
    </span>
  ) : null;
});
