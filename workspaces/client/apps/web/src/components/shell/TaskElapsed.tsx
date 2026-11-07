import type { TaskProps } from "./backgroundTaskPresentation";
import { Clock } from "lucide-react";
import { observer } from "mobx-react-lite";
import { useEffect, useState } from "react";
import { activeTask, type DisplayBackgroundTask } from "../backgroundTaskModel";
import { elapsed } from "./backgroundTaskPresentation";
import { probe } from "./backgroundTaskRenderProbe";
/** Mounted only with the open activity panel; no list or detail observes the clock. */
function useElapsed(task: DisplayBackgroundTask) {
  const [now, setNow] = useState(Date.now() / 1000);
  const ticking = activeTask(task) && !task.finished && task.durationMs == null;
  useEffect(() => {
    if (!ticking) return;
    let timer: ReturnType<typeof setInterval> | undefined;
    const sync = () => {
      clearInterval(timer);
      if (!document.hidden) {
        setNow(Date.now() / 1000);
        timer = setInterval(() => setNow(Date.now() / 1000), 1000);
      }
    };
    sync();
    document.addEventListener("visibilitychange", sync);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", sync);
    };
  }, [ticking]);
  probe("background-task-elapsed", task.id);
  return elapsed(task, now);
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
