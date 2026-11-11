import { useState } from "react";
import { AsyncFeedback } from "../../../../stores/AsyncFeedback.ts";
import type { TaskProps } from "../TaskStatus/TaskStatus.tsx";
import { observer } from "mobx-react-lite";
import { useBackgroundTaskActions } from "../support/backgroundTaskActions.tsx";
import * as API from "../../../../generated/api.ts";
import { inputControls, receiptText } from "../support/taskControlModel.ts";
import { TextInput, NumberInput, Button, Text } from "@mantine/core";
export const ProcessInput = observer(function ProcessInput({
  task,
}: TaskProps) {
  const actions = useBackgroundTaskActions();
  return <InputForm key={actions.leadId} task={task} />;
});
const InputForm = observer(function InputForm({ task }: TaskProps) {
  const [feedback] = useState(() => new AsyncFeedback());
  const actions = useBackgroundTaskActions();
  const [input, setInput] = useState("");
  const [s, setDraft] = useState({
    rows: 24 as string | number,
    cols: 80 as string | number,
    closed: false,
    receipt: "",
  });
  const patch = (value: Partial<typeof s>) =>
    setDraft((current) => ({ ...current, ...value }));
  const controls = inputControls(task, s.closed),
    owner = actions.owner(task.agent),
    pending = actions.pending || feedback.pending;
  const blocked = pending || controls.unavailable;
  return (
    <section className="process-input" aria-label="Command controls">
      {!controls.receiptOnly && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void feedback.run(
              () => actions.sendInput(task, input),
              (ok) =>
                ok && setInput((current) => (current === input ? "" : current)),
            );
          }}
          className="process-input-line"
        >
          <TextInput
            aria-label="Terminal input"
            placeholder="Type terminal input"
            value={input}
            onChange={(e) => setInput(e.currentTarget.value)}
            disabled={blocked}
            maxLength={16000}
          />
          <Button size="sm" type="submit" disabled={blocked}>
            {controls.native ? "Send via agent" : "Send line"}
          </Button>
        </form>
      )}
      <div className="process-input-actions">
        {!controls.native && (
          <Button
            size="compact-xs"
            disabled={pending || !owner}
            onClick={() =>
              void feedback.run(
                () => Promise.resolve(actions.checkInputReceipt(task)),
                (status) =>
                  patch({ receipt: receiptText[status || "unknown"] }),
              )
            }
          >
            Check input receipt
          </Button>
        )}
        {!controls.native && !controls.receiptOnly && (
          <>
            <Button
              size="compact-xs"
              disabled={blocked}
              onClick={() =>
                void actions.act("/api/monitor/input", {
                  id: task.id,
                  text: "\u0003",
                })
              }
            >
              Ctrl+C
            </Button>
            <Button
              size="compact-xs"
              disabled={blocked}
              onClick={() =>
                void feedback.run(
                  () =>
                    actions.act("/api/monitor/input", {
                      id: task.id,
                      closeStdin: true,
                    }),
                  (ok) => {
                    if (ok) patch({ closed: true });
                  },
                )
              }
            >
              {controls.stdinClosed
                ? "Input closed"
                : task.display.stdinState ===
                    API.ProcessStdinState.closeRequested
                  ? "Closing input…"
                  : "Close input (EOF)"}
            </Button>
            <details className="process-resize">
              <summary>Terminal size</summary>
              <div>
                {(["rows", "cols"] as const).map((name) => (
                  <NumberInput
                    key={name}
                    label={name === "rows" ? "Rows" : "Columns"}
                    value={s[name]}
                    onChange={(value) => patch({ [name]: value })}
                    size="xs"
                    min={1}
                    max={500}
                    allowDecimal={false}
                  />
                ))}
                <Button
                  size="xs"
                  disabled={
                    pending ||
                    !task.display.actions.includes(
                      API.BackgroundTaskAction.resize,
                    ) ||
                    !Number(s.rows) ||
                    !Number(s.cols)
                  }
                  onClick={() =>
                    void actions.act("/api/monitor/input", {
                      id: task.id,
                      rows: Number(s.rows),
                      cols: Number(s.cols),
                    })
                  }
                >
                  Resize
                </Button>
              </div>
            </details>
          </>
        )}
      </div>
      {(feedback.error || s.receipt) && (
        <Text size="xs" role="status">
          {feedback.error || s.receipt}
        </Text>
      )}
    </section>
  );
});
