import type { TaskProps } from "./backgroundTaskPresentation";
import { observer } from "mobx-react-lite";
import { activeTask } from "../backgroundTaskModel";
import { BackgroundTaskLabel, BackgroundTaskKind } from "../../generated/api";
import { Text } from "@mantine/core";
import MonitorRestart from "./MonitorRestart";
export const TaskMonitorControls = observer(function TaskMonitorControls({
  task,
}: TaskProps) {
  if (task.display.kind !== BackgroundTaskKind.monitor || !("key" in task))
    return null;
  const budget = task.outputWakeBudget;
  return (
    budget && (
      <div className="task-wake-budget" role="status">
        <Text size="sm">
          {budget.totalSuppressedMatches} matches suppressed
        </Text>
        {budget.paused && (
          <>
            <Text fw={600} size="sm">
              Output wakes paused
            </Text>
            <Text size="sm">
              {task.display.label === BackgroundTaskLabel.running
                ? "The command is still running. Its exit will still wake the agent."
                : "The command exit will still wake the agent."}
            </Text>
          </>
        )}
        {activeTask(task) && <MonitorRestart task={task} />}
      </div>
    )
  );
});
