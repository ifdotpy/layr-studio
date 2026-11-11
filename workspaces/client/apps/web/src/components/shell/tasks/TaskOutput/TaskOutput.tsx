import { TaskToolInput } from "../TaskToolInput/TaskToolInput.tsx";
import type { TaskProps } from "../TaskStatus/TaskStatus.tsx";
import { observer } from "mobx-react-lite";
import { useBackgroundTaskActions } from "../support/backgroundTaskActions.tsx";
import { activeTask } from "../support/backgroundTaskModel.ts";
import { useRef, useEffect, useState } from "react";
import { ActionIcon, Button } from "@mantine/core";
import * as Icons from "lucide-react";
import { AsyncFeedback } from "../../../../stores/AsyncFeedback.ts";
import { CopyFeedback } from "../../../../stores/CopyFeedback.ts";
import { BackgroundTaskAction } from "../../../../generated/api.ts";
import type { GetResult } from "../../../../api.ts";
import ErrorDescription from "../../ErrorDescription/ErrorDescription.tsx";
import { probe } from "../../../../perf/backgroundTaskRenderProbe.ts";

export const TaskOutput = observer(function TaskOutput({ task }: TaskProps) {
  const actions = useBackgroundTaskActions();
  return <OutputView key={actions.leadId} task={task} />;
});

const OutputView = observer(function OutputView({ task }: TaskProps) {
  const [feedback] = useState(() => new AsyncFeedback()),
    [copy] = useState(() => new CopyFeedback());
  const actions = useBackgroundTaskActions();
  const [fileOutput, setFileOutput] =
      useState<GetResult<"/api/task/output"> | null>(null),
    [follow, setFollow] = useState(true);
  const supported = task.display.actions.includes(
      BackgroundTaskAction.readOutput,
    ),
    tail = supported ? (fileOutput?.text ?? task.tail) : undefined;
  const output = useRef<HTMLPreElement>(null);
  useEffect(() => {
    if (follow && output.current)
      output.current.scrollTop = output.current.scrollHeight;
  }, [tail, follow]);

  probe("background-task-output", task.id);
  return (
    <>
      {task.outputFile && (
        <div className="task-file-output">
          <p className="task-cwd">{task.outputFile}</p>
          <Button
            size="xs"
            loading={feedback.pending}
            onClick={() =>
              void feedback.run(() => actions.readOutput(task), setFileOutput)
            }
          >
            View output
          </Button>
          {feedback.error && (
            <p className="task-error" role="alert">
              {feedback.error}
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
              <Icons.LoaderCircle size={12} className="spin" />
            ) : (
              <Icons.Terminal size={12} />
            )}
            {supported ? "Output" : "Output unavailable"}
          </span>
          {supported && (
            <div>
              <Button
                variant="subtle"
                size="compact-xs"
                aria-pressed={follow}
                onClick={() => setFollow(!follow)}
                leftSection={<Icons.ChevronDown size={12} />}
                color={follow ? "ink" : "gray"}
              >
                Follow
              </Button>
              <ActionIcon
                size="sm"
                aria-label="Download task log"
                disabled={actions.pending || (!!task.outputFile && !fileOutput)}
                onClick={() => void actions.downloadLog(task, fileOutput?.text)}
              >
                <Icons.Download size={13} />
              </ActionIcon>
              <ActionIcon
                size="sm"
                aria-label="Copy task output"
                disabled={!tail}
                onClick={() => void actions.copyOutput(copy, tail || "")}
              >
                {copy.copied() ? (
                  <Icons.Check size={13} />
                ) : (
                  <Icons.Copy size={13} />
                )}
              </ActionIcon>
            </div>
          )}
        </div>
        <pre
          ref={output}
          className={`task-output ${!tail ? "empty" : ""}`}
          onScroll={(e) => {
            const el = e.currentTarget;
            if (el.scrollHeight - el.scrollTop - el.clientHeight > 35)
              setFollow(false);
          }}
        >
          {!supported
            ? "This provider does not expose process output."
            : tail ||
              (task.outputFile && !fileOutput
                ? "Choose View output to read this file."
                : activeTask(task)
                  ? "Waiting for output…"
                  : "No output recorded.")}
        </pre>
        {(task.outputTruncated ||
          (("bytes" in task ? task.bytes : 0) || 0) >
            new TextEncoder().encode(tail || "").length) && (
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
