import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { observable } from "mobx";
import { capacityProjection } from "../support/recovery.ts";
import ErrorDescription from "../../shell/ErrorDescription/ErrorDescription.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { AsyncFeedback } from "../../../stores/AsyncFeedback.ts";
import { Button } from "@mantine/core";
import { errorText, type PostResult } from "../../../api.ts";
import type { CapacityRetry as CapacityRetryRecord } from "../../../capacityRetry.ts";

export default function CapacityRetry({
  agentId,
  retry,
}: {
  agentId: string;
  retry: CapacityRetryRecord;
}) {
  const { post } = useServerApi();

  return (
    <Owned
      create={() => ({
        action: new AsyncFeedback(),
        confirmed: observable.box<PostResult<"/api/capacity-retry">>(),
      })}
    >
      {(state) => {
        const { current, scheduled, message, available } = capacityProjection(
          retry,
          state.confirmed.get(),
        );
        const action = (choice: "retry" | "cancel") =>
          state.action.run(
            () =>
              post(
                "/api/capacity-retry",
                { id: agentId, retry_id: retry.id, action: choice },
                { timeoutMs: 15000 },
              ).catch((cause) => {
                throw new Error(
                  (choice === "cancel"
                    ? "Cancel could not be confirmed. The automatic retry may still start. "
                    : "Retry could not be confirmed. Check the chat status. ") +
                    errorText(cause),
                );
              }),
            (result) => state.confirmed.set(result),
          );
        const choices = scheduled
          ? (["retry", "cancel"] as const)
          : (["retry"] as const);
        return (
          <div
            className="capacity-retry"
            data-retry-status={current.status || "unknown"}
          >
            <p role="status">{message}</p>
            {current.reason && (
              <p>
                <ErrorDescription value={current.reason} role="status" />
              </p>
            )}
            {available && (
              <div className="native-error-actions">
                {choices.map((choice) => (
                  <Button
                    key={choice}
                    size="compact-xs"
                    variant={choice === "cancel" ? "subtle" : "filled"}
                    disabled={state.action.pending}
                    onClick={() => void action(choice)}
                  >
                    {choice === "retry"
                      ? "Retry now"
                      : "Cancel automatic retry"}
                  </Button>
                ))}
              </div>
            )}
            {state.action.error && <p role="alert">{state.action.error}</p>}
          </div>
        );
      }}
    </Owned>
  );
}
