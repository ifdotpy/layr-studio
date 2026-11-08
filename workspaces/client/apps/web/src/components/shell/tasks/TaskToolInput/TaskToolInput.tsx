import { resourceData } from "../../../../store/runtime.ts";
import type { TaskProps } from "../TaskStatus/TaskStatus.tsx";
import { observer } from "mobx-react-lite";
import { BackgroundTaskKind } from "../../../../generated/api.ts";
import { useState } from "react";
import { Button } from "@mantine/core";
import { errorText } from "../../../../api.ts";
import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { taskId } from "../../../../generated/observableResources.ts";

export const TaskToolInput = observer(function TaskToolInput({
  task,
}: TaskProps) {
  const store = useServerStore();
  const [requested, request] = useState(false);
  const slot = requested
    ? store.taskDetail(taskId(task.id)).current
    : undefined;
  const detail = slot ? resourceData(slot) : undefined;
  return (
    <>
      {task.display.kind !== BackgroundTaskKind.monitor && !detail && (
        <Button
          size="compact-xs"
          variant="subtle"
          onClick={() => {
            request(true);
          }}
        >
          Read tool input
        </Button>
      )}
      {detail?.arguments && (
        <details className="task-input">
          <summary>Tool input</summary>
          <pre>{detail.arguments}</pre>
        </details>
      )}
      {slot?.status === "error" && (
        <p role="alert" className="task-error">
          Output unavailable: {errorText(slot.error)}
        </p>
      )}
    </>
  );
});
