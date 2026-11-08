import { BackgroundTaskStrip } from "../BackgroundTaskStrip/BackgroundTaskStrip.tsx";
import { observer } from "mobx-react-lite";
import { TextInput } from "@mantine/core";
import { Search, Check } from "lucide-react";
import { useBackgroundTaskActions } from "../support/backgroundTaskActions.tsx";
import { probe } from "../../../../perf/backgroundTaskRenderProbe.ts";

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
