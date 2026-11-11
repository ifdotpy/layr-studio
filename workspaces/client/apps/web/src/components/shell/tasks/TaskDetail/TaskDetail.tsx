import { resourceData } from "../../../../store/runtime.ts";
import { BackgroundTaskKind } from "../../../../generated/api.ts";
import { observer, Observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import * as Icons from "lucide-react";
import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { localDateTime } from "../../../../local-time.ts";
import { activeTask } from "../support/backgroundTaskModel.ts";
import { useBackgroundTaskActions } from "../support/backgroundTaskActions.tsx";
import {
  TaskStatus,
  kindLabels,
  duration,
  type TaskProps,
} from "../TaskStatus/TaskStatus.tsx";
import { TaskElapsedFact } from "../TaskElapsed/TaskElapsed.tsx";
import { TaskOutput } from "../TaskOutput/TaskOutput.tsx";
import { TaskMonitorControls } from "../TaskMonitorControls/TaskMonitorControls.tsx";
import { ProcessInput } from "../ProcessInput/ProcessInput.tsx";
import { probe } from "../../../../perf/backgroundTaskRenderProbe.ts";
export const TaskDetail = observer(function TaskDetail({ task }: TaskProps) {
  const actions = useBackgroundTaskActions(),
    store = useServerStore();
  probe("background-task-detail", task.id);
  return (
    <section
      className="task-detail"
      aria-label="Task details"
      data-task-detail={task.id}
    >
      <div className="task-detail-top">
        <Button
          variant="subtle"
          className="task-back"
          size="compact-xs"
          leftSection={<Icons.ArrowLeft size={14} />}
          onClick={actions.back}
        >
          Activity
        </Button>
        <Observer>
          {() => {
            const Icon =
              task.display.kind === BackgroundTaskKind.monitor
                ? Icons.Activity
                : task.display.kind === BackgroundTaskKind.command
                  ? Icons.Terminal
                  : Icons.Wrench;
            return (
              <span>
                <Icon size={15} />
                {kindLabels[task.display.kindLabel]}
              </span>
            );
          }}
        </Observer>
        <TaskStatus task={task} />
      </div>
      <div className="task-detail-scroll">
        <Observer>
          {() => (
            <h2 className={task.command ? "task-command-title" : ""}>
              {task.display.title}
            </h2>
          )}
        </Observer>
        <Observer>
          {() => (
            <Button
              variant="subtle"
              size="compact-xs"
              className="task-owner"
              rightSection={<Icons.ArrowUpRight size={13} />}
              onClick={() => actions.open(task)}
            >
              {actions.owner(task.agent)?.name || "Open agent"}
            </Button>
          )}
        </Observer>
        <Observer>
          {() => (
            <div className="task-facts">
              <TaskElapsedFact task={task} />
              {task.exitCode != null && (
                <span className={task.exitCode !== 0 ? "task-failed" : ""}>
                  Exit {task.exitCode}
                </span>
              )}
              {"timeout_ms" in task && task.timeout_ms != null && (
                <span>Timeout {duration(task.timeout_ms / 1000)}</span>
              )}
              {task.processId && <span>Process {task.processId}</span>}
            </div>
          )}
        </Observer>
        <Observer>
          {() => task.cwd && <p className="task-cwd">{task.cwd}</p>}
        </Observer>
        <Observer>
          {() =>
            task.display.kind === BackgroundTaskKind.monitor &&
            activeTask(task) && (
              <p className="task-note">
                {task.display.actions.includes("approve")
                  ? "The command waits for your approval."
                  : "The agent receives the result when this command exits."}
              </p>
            )
          }
        </Observer>
        <TaskMonitorControls task={task} />
        <TaskOutput task={task} />
        <Observer>
          {() =>
            (task.display.kind === BackgroundTaskKind.monitor ||
              task.display.actions.includes("input")) && (
              <ProcessInput task={task} />
            )
          }
        </Observer>
        <Observer>
          {() => (
            <dl className="task-record">
              {(
                [
                  ["Created", localDateTime(new Date(task.created * 1000))],
                  [
                    "Finished",
                    task.finished &&
                      localDateTime(new Date(task.finished * 1000)),
                  ],
                  ["Saved log", "log" in task && task.log],
                ] as const
              ).map(
                ([label, value]) =>
                  value && (
                    <div key={label}>
                      <dt>{label}</dt>
                      <dd>{value}</dd>
                    </div>
                  ),
              )}
            </dl>
          )}
        </Observer>
      </div>
      <Observer>
        {() => {
          const current = task.display.actions.includes("approve")
            ? resourceData(store.requests().current)
            : undefined;
          const request = current
            ? [...current.requests.values()].find(
                (r) =>
                  r.request.kind === "monitor_start" &&
                  r.request.monitor === task.id,
              )
            : undefined;
          return (
            <div className="task-actions">
              {request && (
                <Button
                  size="xs"
                  variant="light"
                  loading={actions.pending}
                  onClick={() => void actions.approve(request.id)}
                >
                  Approve command
                </Button>
              )}
              {actions.canStop && (
                <Button
                  variant="light"
                  size="xs"
                  color="red"
                  leftSection={<Icons.Square size={12} />}
                  loading={actions.controlPending}
                  disabled={actions.controlPending}
                  onClick={actions.stopSelected}
                >
                  Stop selected process
                </Button>
              )}
            </div>
          );
        }}
      </Observer>
    </section>
  );
});
