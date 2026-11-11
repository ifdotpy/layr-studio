import { useState } from "react";
import { flowResult } from "mobx";
import { AsyncFeedback } from "../../../../stores/AsyncFeedback.ts";
import { Button, Text, Textarea, TextInput } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { useBackgroundTaskActions } from "../support/backgroundTaskActions.tsx";
import { restartPatterns } from "../support/taskControlModel.ts";
export { restartPatterns } from "../support/taskControlModel.ts";
import type { DisplayProcessTask } from "../support/backgroundTaskModel.ts";

const MonitorRestart = observer(function MonitorRestart({
  task,
}: {
  task: DisplayProcessTask;
}) {
  const actions = useBackgroundTaskActions();
  return <RestartForm key={actions.leadId} task={task} />;
});
const RestartForm = observer(function RestartForm({
  task,
}: {
  task: DisplayProcessTask;
}) {
  const [feedback] = useState(() => new AsyncFeedback());
  const actions = useBackgroundTaskActions(),
    receipt = actions.restartReceipt(task.id);
  const [opened, setOpened] = useState(!!receipt),
    [patterns, setPatterns] = useState(task.outputPatterns.join("\n"));
  const values = restartPatterns(patterns),
    unchanged = JSON.stringify(values) === JSON.stringify(task.outputPatterns);
  if (!opened && task.cancelRequested) return null;
  if (!opened)
    return (
      <Button size="xs" variant="light" mt="xs" onClick={() => setOpened(true)}>
        Restart with narrower patterns
      </Button>
    );
  return (
    <div className="task-restart">
      <Text fw={600} size="sm">
        Restart with narrower patterns
      </Text>
      <TextInput label="Command" value={task.command || ""} readOnly />
      <Textarea
        label="Output patterns, one per line"
        value={receipt ? receipt.outputPatterns.join("\n") : patterns}
        disabled={feedback.pending || !!receipt}
        onChange={(event) => setPatterns(event.currentTarget.value)}
        autosize
        minRows={2}
      />
      <Text size="sm">
        Use more specific patterns. Confirm to stop the old command and run this
        command again after it exits.
      </Text>
      {feedback.error && (
        <Text role="alert" c="red" size="sm">
          {feedback.error}
        </Text>
      )}
      <div className="task-restart-actions">
        <Button
          size="xs"
          loading={feedback.pending}
          disabled={!receipt && (!values.length || unchanged)}
          onClick={() =>
            void feedback.run(
              () => flowResult(actions.restartWithPatterns(task, patterns)),
              () => setOpened(false),
            )
          }
        >
          {receipt ? "Check restart result" : "Confirm restart"}
        </Button>
        <Button
          size="xs"
          variant="subtle"
          disabled={feedback.pending || !!receipt}
          onClick={() => setOpened(false)}
        >
          Cancel
        </Button>
      </div>
    </div>
  );
});
export default MonitorRestart;
