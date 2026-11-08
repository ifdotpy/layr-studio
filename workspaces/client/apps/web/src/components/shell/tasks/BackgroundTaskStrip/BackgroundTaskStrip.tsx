import { observer, Observer } from "mobx-react-lite";
import { UnstyledButton } from "@mantine/core";
import { useBackgroundTaskActions } from "../support/backgroundTaskActions.tsx";
import {
  TaskStatus,
  kindLabels,
  type TaskProps,
} from "../TaskStatus/TaskStatus.tsx";
import { TaskElapsed } from "../TaskElapsed/TaskElapsed.tsx";
import { probe } from "../../../../perf/backgroundTaskRenderProbe.ts";

export const BackgroundTaskStrip = observer(function BackgroundTaskStrip({
  task,
}: TaskProps) {
  const actions = useBackgroundTaskActions();
  const selected = actions.selectedEntry?.value === task;
  probe("background-task-row", task.id);
  return (
    <UnstyledButton
      data-task={task.id}
      data-studio-state={task.display.indicator}
      className={`task-row ${selected ? "selected" : ""}`}
      aria-pressed={selected}
      onClick={() => actions.select(task)}
    >
      <div className="task-row-heading">
        <Observer>
          {() => (
            <span className="task-row-kind">
              {kindLabels[task.display.kindLabel]}
            </span>
          )}
        </Observer>
        <span className="task-age">
          <TaskElapsed task={task} />
        </span>
      </div>
      <Observer>
        {() => {
          probe("background-task-title", task.id);
          return (
            <strong
              className={task.command ? "task-command-title" : ""}
              title={task.display.title}
            >
              {task.display.title}
            </strong>
          );
        }}
      </Observer>
      <div className="task-row-footer">
        <Observer>
          {() => {
            probe("background-task-owner", task.id);
            return <span>{actions.owner(task.agent)?.name || "Agent"}</span>;
          }}
        </Observer>
        <TaskStatus task={task} />
      </div>
    </UnstyledButton>
  );
});
