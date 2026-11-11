import { observer } from "mobx-react-lite";
import { Button, Checkbox } from "@mantine/core";
import { Square } from "lucide-react";
import { useBackgroundTaskActions } from "../support/backgroundTaskActions.tsx";

export const ProcessControlActions = observer(function ProcessControlActions() {
  const actions = useBackgroundTaskActions();
  const blocked = !!actions.pendingControl;
  const owned =
    blocked ||
    actions.current.status === "loading" ||
    !!actions.processLoadError ||
    !actions.processTasks.some(
      (process) =>
        process.display.actions.includes("stop") &&
        (actions.includeWorkers || process.agent === actions.leadId),
    );
  return (
    <div className="tasks-process-actions" aria-label="Process controls">
      <Checkbox
        size="xs"
        label="Include worker processes"
        checked={actions.includeWorkers}
        onChange={(event) =>
          actions.setIncludeWorkers(event.currentTarget.checked)
        }
        disabled={actions.controlPending || blocked}
      />
      {(["stop_response", "stop_owned"] as const).map((action) => (
        <Button
          key={action}
          variant="light"
          size="xs"
          color="red"
          leftSection={<Square size={12} />}
          loading={actions.controlPending}
          disabled={
            action === "stop_response"
              ? !actions.listing?.responseTarget || blocked
              : owned
          }
          onClick={() => actions.runProcessControl({ action })}
        >
          {action === "stop_response"
            ? "Stop response"
            : "Stop owned processes"}
        </Button>
      ))}
      {actions.pendingControl && (
        <Button
          size="xs"
          variant="light"
          loading={actions.controlPending}
          onClick={() => void actions.control(true)}
        >
          Check stop result
        </Button>
      )}
    </div>
  );
});
