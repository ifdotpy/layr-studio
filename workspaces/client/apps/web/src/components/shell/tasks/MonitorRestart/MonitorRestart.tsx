import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { Button, Text, Textarea, TextInput } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { useBackgroundTaskActions } from "../support/backgroundTaskActions.tsx";
import { useState } from "react";
import type { DisplayProcessTask } from "../support/backgroundTaskModel.ts";

export function restartPatterns(text: string): string[] {
  return text.split("\n").filter((line) => line.length > 0);
}

const MonitorRestart = observer(function MonitorRestart({
  task,
}: {
  task: DisplayProcessTask;
}) {
  const actions = useBackgroundTaskActions();
  const [receipt, setReceipt] = useState(() => actions.restartReceipt(task.id));
  const [opened, setOpened] = useState(!!receipt);
  const { outputPatterns, epoch, accountKey } = task;
  const [patterns, setPatterns] = useState(outputPatterns.join("\n"));
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const values = restartPatterns(patterns);
  const unchanged =
    patterns === outputPatterns.join("\n") ||
    JSON.stringify(values) === JSON.stringify(outputPatterns);
  const submit = async () => {
    if (pending) return;
    const body = receipt || {
      id: task.id,
      agent: task.agent,
      epoch,
      accountKey: accountKey || "default",
      requestId: randomUUID(),
      outputPatterns: values,
    };
    setReceipt(body);
    setPending(true);
    setError("");
    const result = await actions.restart(body);
    if (result.status === "applied") {
      setReceipt(null);
      setOpened(false);
    } else {
      if (result.status === "rejected") setReceipt(null);
      setError(result.error);
    }
    setPending(false);
  };
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
        disabled={pending || !!receipt}
        onChange={(event) => setPatterns(event.currentTarget.value)}
        autosize
        minRows={2}
      />
      <Text size="sm">
        Use more specific patterns. Confirm to stop the old command and run this
        command again after it exits.
      </Text>
      {error && (
        <Text role="alert" c="red" size="sm">
          {error}
        </Text>
      )}
      <div className="task-restart-actions">
        <Button
          size="xs"
          loading={pending}
          disabled={!receipt && (!values.length || unchanged)}
          onClick={() => void submit()}
        >
          {receipt ? "Check restart result" : "Confirm restart"}
        </Button>
        <Button
          size="xs"
          variant="subtle"
          disabled={pending || !!receipt}
          onClick={() => setOpened(false)}
        >
          Cancel
        </Button>
      </div>
    </div>
  );
});
export default MonitorRestart;
