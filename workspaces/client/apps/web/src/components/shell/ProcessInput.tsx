import type { TaskProps } from "./backgroundTaskPresentation";
import { observer } from "mobx-react-lite";
import { useBackgroundTaskActions } from "./backgroundTaskActions";
import { BackgroundTaskAction, BackgroundTaskKind } from "../../generated/api";
import { useState } from "react";
import { TextInput, NumberInput, Button, Text } from "@mantine/core";
import { displayError } from "../../errorPresentation";
import {
  ProcessStdinState,
  NativeCommandAction as GeneratedApiNativeCommandAction,
} from "../../generated/api";
export const ProcessInput = observer(function ProcessInput({
  task,
}: TaskProps) {
  const actions = useBackgroundTaskActions();
  const { pending, act } = actions;
  const owner = actions.owner(task.agent);
  const receiptOnly =
    !task.display.actions.includes(BackgroundTaskAction.input) &&
    !task.display.actions.includes(BackgroundTaskAction.resize);
  const [input, setInput] = useState("");
  const [rows, setRows] = useState<string | number>(24);
  const [cols, setCols] = useState<string | number>(80);
  const [inputClosed, setStdinClosed] = useState(false);
  const [receiptState, setReceiptState] = useState("");
  const [checkingReceipt, setCheckingReceipt] = useState(false);
  const stdinClosed =
    inputClosed || task.display.stdinState === ProcessStdinState.closed;
  const stdinUnavailable =
    stdinClosed || !task.display.actions.includes(BackgroundTaskAction.input);
  const blocked = pending || checkingReceipt || stdinUnavailable;
  const native = task.display.kind === BackgroundTaskKind.command;
  const checkReceipt = async () => {
    if (!owner || owner.epoch == null || checkingReceipt || pending) return;
    setCheckingReceipt(true);
    try {
      const status = await actions.checkInputReceipt(task);
      setReceiptState(
        {
          acknowledged: "The process acknowledged the input.",
          rejected: "The process rejected the input.",
          none: "No input request needs a receipt check.",
          unknown: "The input result is unknown. Do not send different input.",
        }[status || "unknown"],
      );
    } catch (error) {
      setReceiptState(displayError(error));
    } finally {
      setCheckingReceipt(false);
    }
  };
  const send = async () => {
    if (pending || checkingReceipt || receiptOnly || stdinUnavailable) return;
    const sent = input;
    const ok = native
      ? await act("/api/native-command", {
          id: task.id,
          action: GeneratedApiNativeCommandAction.input,
          text: sent + "\n",
        })
      : await act("/api/monitor/input", {
          id: task.id,
          text: sent + "\n",
        });
    if (ok) setInput((current) => (current === sent ? "" : current));
  };
  return (
    <section className="process-input" aria-label="Command controls">
      {!receiptOnly && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void send();
          }}
          className="process-input-line"
        >
          <TextInput
            aria-label="Terminal input"
            placeholder="Type terminal input"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            disabled={blocked}
            maxLength={16000}
          />
          <Button size="sm" type="submit" disabled={blocked}>
            {native ? "Send via agent" : "Send line"}
          </Button>
        </form>
      )}
      <div className="process-input-actions">
        {!native && (
          <Button
            size="compact-xs"
            disabled={pending || checkingReceipt || !owner}
            onClick={() => void checkReceipt()}
          >
            Check input receipt
          </Button>
        )}
        {!native && !receiptOnly && (
          <>
            <Button
              size="compact-xs"
              disabled={blocked}
              onClick={() =>
                void act("/api/monitor/input", { id: task.id, text: "\u0003" })
              }
            >
              Ctrl+C
            </Button>
            <Button
              size="compact-xs"
              disabled={blocked}
              onClick={() => {
                void act("/api/monitor/input", {
                  id: task.id,
                  closeStdin: true,
                }).then((ok) => {
                  if (ok) setStdinClosed(true);
                });
              }}
            >
              {stdinClosed
                ? "Input closed"
                : task.display.stdinState === ProcessStdinState.closeRequested
                  ? "Closing input…"
                  : "Close input (EOF)"}
            </Button>
            <details className="process-resize">
              <summary>Terminal size</summary>
              <div>
                {(
                  [
                    ["Rows", rows, setRows],
                    ["Columns", cols, setCols],
                  ] as const
                ).map(([label, value, onChange]) => (
                  <NumberInput
                    key={label}
                    {...{ label, value, onChange }}
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
                    checkingReceipt ||
                    !task.display.actions.includes(
                      BackgroundTaskAction.resize,
                    ) ||
                    !Number(rows) ||
                    !Number(cols)
                  }
                  onClick={() =>
                    void act("/api/monitor/input", {
                      id: task.id,
                      rows: Number(rows),
                      cols: Number(cols),
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
      {receiptState && (
        <Text size="xs" role="status">
          {receiptState}
        </Text>
      )}
    </section>
  );
});
