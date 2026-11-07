import { observer, Observer } from "mobx-react-lite";
import { UnstyledButton, TextInput } from "@mantine/core";
import { Search, Check } from "lucide-react";
import { useBackgroundTaskActions } from "./backgroundTaskActions";
import {
  TaskStatus,
  kindLabels,
  type TaskProps,
} from "./backgroundTaskPresentation";
import { TaskElapsed } from "./TaskElapsed";
import { probe } from "./backgroundTaskRenderProbe";
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
export const BackgroundTaskList = observer(function BackgroundTaskList() {
  const actions = useBackgroundTaskActions();
  probe("background-task-list");
  return (
    <section className="tasks-list" aria-label="Task list">
      <div className="tasks-search">
        <TextInput
          size="sm"
          aria-label="Find a task"
          placeholder="Search commands or agents"
          leftSection={<Search size={14} />}
          value={actions.query}
          onChange={(e) => actions.filter(actions.kind, e.target.value)}
        />
      </div>
      <div className="tasks-rows">
        {actions.filtered.map(({ source, value: task }) => (
          <BackgroundTaskStrip key={`${source}:${task.id}`} task={task} />
        ))}
        {!actions.filtered.length && (
          <div className="tasks-empty">
            <Check size={24} />
            <strong>
              {actions.query ? "No matching tasks" : "No active tasks"}
            </strong>
            <p>
              {actions.query
                ? "Try another command or agent name."
                : "Agent commands and tool calls appear here."}
            </p>
          </div>
        )}
      </div>
    </section>
  );
});
