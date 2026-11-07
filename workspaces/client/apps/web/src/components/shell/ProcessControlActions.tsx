import { ProcessActionStatus } from "../../generated/api";
import { observer } from "mobx-react-lite";
import { Button, Checkbox } from "@mantine/core";
import { Square } from "lucide-react";
import { runInAction } from "mobx";
import { useBackgroundTaskActions } from "./backgroundTaskActions";
const errorSuffix = (value: object) =>
  "error" in value && typeof value.error === "string" && value.error
    ? `, ${value.error}`
    : "";
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
          runInAction(() => {
            actions.includeWorkers = event.currentTarget.checked;
          })
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
export const ProcessControlResults = observer(function ProcessControlResults() {
  const actions = useBackgroundTaskActions();
  const result = actions.controlResult;
  return (
    <>
      {actions.processLoadError && (
        <p role="alert" className="task-error">
          Process list unavailable: {actions.processLoadError}
        </p>
      )}
      {actions.current.status === "loading" && (
        <p role="status">Loading processes…</p>
      )}
      {result?.response && (
        <p
          role={
            result.status === ProcessActionStatus.uncertain ? "alert" : "status"
          }
        >
          Response stop: {String(result.response.status)}
          {errorSuffix(result.response)}
        </p>
      )}
      {!!result?.processes?.length && (
        <ul
          aria-label="Process stop results"
          className="process-control-results"
        >
          {result.processes.map((item, index) => (
            <li key={`${item.key}:${index}`}>
              {actions.processTasks.find((process) => process.key === item.key)
                ?.name ||
                actions.owner(item.agent)?.name ||
                "Process"}
              : {item.status}
              {errorSuffix(item)}
            </li>
          ))}
        </ul>
      )}
    </>
  );
});
