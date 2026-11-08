import { ProcessActionStatus } from "../../../../generated/api.ts";
import { observer } from "mobx-react-lite";
import { useBackgroundTaskActions } from "../support/backgroundTaskActions.tsx";

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

const errorSuffix = (value: object) =>
  "error" in value && typeof value.error === "string" && value.error
    ? `, ${value.error}`
    : "";
