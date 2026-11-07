import { BackgroundTaskKind } from "../../generated/api";
import { observer, Observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import {
  ArrowLeft,
  ArrowUpRight,
  Square,
  Activity,
  Terminal,
  Wrench,
} from "lucide-react";
import { useServerStore } from "../../servers/ServerServicesContext";
import { localDateTime } from "../../local-time";
import { activeTask } from "../backgroundTaskModel";
import { useBackgroundTaskActions } from "./backgroundTaskActions";
import {
  TaskStatus,
  kindLabels,
  duration,
  type TaskProps,
} from "./backgroundTaskPresentation";
import { TaskElapsedFact } from "./TaskElapsed";
import { TaskOutput } from "./TaskOutput";
import { TaskMonitorControls } from "./TaskMonitorControls";
import { ProcessInput } from "./ProcessInput";
import { probe } from "./backgroundTaskRenderProbe";
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
          leftSection={<ArrowLeft size={14} />}
          onClick={actions.back}
        >
          Activity
        </Button>
        <Observer>
          {() => {
            const Icon =
              task.display.kind === BackgroundTaskKind.monitor
                ? Activity
                : task.display.kind === BackgroundTaskKind.command
                  ? Terminal
                  : Wrench;
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
              rightSection={<ArrowUpRight size={13} />}
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
          const current = store.requests().current;
          const request =
            current.status === "live" || current.status === "stale"
              ? [...current.data.requests.values()].find(
                  (r) =>
                    r.request.kind === "monitor_start" &&
                    r.request.monitor === task.id,
                )
              : undefined;
          return (
            <div className="task-actions">
              {request && task.display.actions.includes("approve") && (
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
                  leftSection={<Square size={12} />}
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
