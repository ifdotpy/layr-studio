import { resourceData } from "../../store/runtime.ts";
import type { TaskProps } from "./backgroundTaskPresentation";
import { observer } from "mobx-react-lite";
import { useBackgroundTaskActions } from "./backgroundTaskActions";
import { activeTask } from "../backgroundTaskModel";
import { BackgroundTaskAction, BackgroundTaskKind } from "../../generated/api";
import { useState, useRef, useEffect } from "react";
import { ActionIcon, Button } from "@mantine/core";
import {
  ChevronDown,
  Check,
  Copy,
  Download,
  LoaderCircle,
  Terminal,
} from "lucide-react";
import { copyText } from "../../clipboard/clipboard";
import ErrorDescription from "../ErrorDescription";
import { errorText } from "../../api";
import { useServerStore } from "../../servers/ServerServicesContext";
import { taskId } from "../../generated/observableResources";
import { probe } from "./backgroundTaskRenderProbe";
export const TaskOutput = observer(function TaskOutput({ task }: TaskProps) {
  const [fileOutput, setFileOutput] = useState<{
    text: string;
    truncated: boolean;
  } | null>(null);
  const [filePending, setFilePending] = useState(false);
  const [fileError, setFileError] = useState("");
  const outputSupported = task.display.actions.includes(
    BackgroundTaskAction.readOutput,
  );
  const outputTail = outputSupported
    ? (fileOutput?.text ?? task.tail)
    : undefined;
  const actions = useBackgroundTaskActions();
  const { pending, notify } = actions;
  const [follow, setFollow] = useState(true),
    [copied, setCopied] = useState(false);
  const output = useRef<HTMLPreElement>(null);
  useEffect(() => {
    if (follow && output.current)
      output.current.scrollTop = output.current.scrollHeight;
  }, [outputTail, follow]);

  probe("background-task-output", task.id);
  return (
    <>
      {task.outputFile && (
        <div className="task-file-output">
          <p className="task-cwd">{task.outputFile}</p>
          <Button
            size="xs"
            loading={filePending}
            onClick={async () => {
              setFilePending(true);
              setFileError("");
              try {
                setFileOutput(await actions.readOutput(task));
              } catch (error) {
                setFileError(errorText(error));
              } finally {
                setFilePending(false);
              }
            }}
          >
            View output
          </Button>
          {fileError && (
            <p className="task-error" role="alert">
              {fileError}
            </p>
          )}
          {fileOutput?.truncated && (
            <p className="task-note">
              The output file exceeds the preview limit. Only its first part
              appears below.
            </p>
          )}
        </div>
      )}
      <TaskToolInput task={task} />
      {[task.error, "stdinError" in task ? task.stdinError : undefined].map(
        (error, index) =>
          error && (
            <p key={index} role="alert" className="task-error">
              <ErrorDescription value={error} />
            </p>
          ),
      )}
      <div className="task-terminal">
        <div className="task-terminal-header">
          <span>
            {activeTask(task) ? (
              <LoaderCircle size={12} className="spin" />
            ) : (
              <Terminal size={12} />
            )}
            {outputSupported ? "Output" : "Output unavailable"}
          </span>
          {outputSupported && (
            <div>
              <Button
                variant="subtle"
                size="compact-xs"
                aria-pressed={follow}
                onClick={() => setFollow(!follow)}
                leftSection={<ChevronDown size={12} />}
                color={follow ? "ink" : "gray"}
              >
                Follow
              </Button>
              <ActionIcon
                size="sm"
                aria-label="Download task log"
                disabled={pending || (!!task.outputFile && !fileOutput)}
                onClick={() => void actions.downloadLog(task, fileOutput?.text)}
              >
                <Download size={13} />
              </ActionIcon>
              <ActionIcon
                size="sm"
                aria-label="Copy task output"
                disabled={!outputTail}
                onClick={() => {
                  void copyText(outputTail || "")
                    .then(() => setCopied(true))
                    .catch(() => notify("Could not copy output"));
                }}
              >
                {copied ? <Check size={13} /> : <Copy size={13} />}
              </ActionIcon>
            </div>
          )}
        </div>
        <pre
          ref={output}
          className={`task-output ${!outputTail ? "empty" : ""}`}
          onScroll={(e) => {
            const el = e.currentTarget;
            if (el.scrollHeight - el.scrollTop - el.clientHeight > 35)
              setFollow(false);
          }}
        >
          {!outputSupported
            ? "This provider does not expose process output."
            : outputTail ||
              (task.outputFile && !fileOutput
                ? "Select View output to read this file."
                : activeTask(task)
                  ? "Waiting for output…"
                  : "No output recorded.")}
        </pre>
        {(task.outputTruncated ||
          (("bytes" in task ? task.bytes : 0) || 0) >
            new TextEncoder().encode(outputTail || "").length) && (
          <p className="task-output-limit">
            Latest output shown
            {("log" in task ? task.log : undefined)
              ? ". The saved log is available below."
              : "."}
          </p>
        )}
      </div>
    </>
  );
});

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
