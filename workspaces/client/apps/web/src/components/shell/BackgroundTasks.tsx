import type { DeepReadonly } from "../../deepReadonly";
import { BackgroundTaskAction, ProcessStdinState } from "../../generated/api";
import { BackgroundTaskKind, BackgroundTaskLabel } from "../../generated/api";
import {
  ProcessActionStatus as GeneratedApiProcessActionStatus,
  NativeCommandAction as GeneratedApiNativeCommandAction,
} from "../../generated/api.ts";
import { useSubscription } from "../../sync/subscriptions";
import { randomUUID } from "../../uuid";
import { sessionOutboxRequests } from "../../browserPersistence";
import {
  useServerApi,
  useServerServices,
} from "../../servers/ServerServicesContext";
import { localDateTime } from "../../local-time";
import ErrorDescription from "../ErrorDescription";
import { displayError } from "../../errorPresentation";
import {
  ActionIcon,
  Badge,
  Button,
  Checkbox,
  Drawer,
  NativeSelect,
  NumberInput,
  Text,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import {
  Activity,
  ArrowLeft,
  ArrowUpRight,
  Check,
  ChevronDown,
  Clock,
  Copy,
  Download,
  LoaderCircle,
  Search,
  Square,
  Terminal,
  Wrench,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import {
  apiDownload,
  ApiError,
  errorText,
  type ApiPostPath,
  type PostBody,
  type PostInput,
  type PostResult,
} from "../../api";
import type { components, paths } from "../../generated/api";
import "./background-controls.css";
import MonitorRestart from "./MonitorRestart";
import type { ChatIndicatorKind } from "../chat-status/chatStatusModel";
import type {
  Agent,
  BackgroundTask,
  Snapshot,
  WorkspaceTask,
} from "../../types";
import type { Approval } from "../../approvals";
import { copyText } from "../../clipboard/clipboard";
import {
  activeTask,
  processIdentity,
  type DisplayBackgroundTask,
  type DisplayProcessTask,
} from "../backgroundTaskModel";

type BackgroundAction = <Path extends ApiPostPath>(
  path: Path,
  body: PostInput<Path>,
) => Promise<boolean>;
type TaskDetailResponse =
  paths["/api/task"]["get"]["responses"][200]["content"]["application/json"];
type PendingRequest = Approval;
type ProcessResponseTarget = NonNullable<
  paths["/api/processes"]["get"]["responses"][200]["content"]["application/json"]["responseTarget"]
>;
export const responseStopIdentity = (target: ProcessResponseTarget) => ({
  responseTarget: target,
});
export const responseStopDisabled = (
  target: ProcessResponseTarget | null | undefined,
  pending: boolean,
) => !target || pending;
export const responseOwnerIdentityKey = (
  owner:
    | Pick<
        Agent,
        "id" | "accountKey" | "epoch" | "inFlight" | "threadId" | "turnId"
      >
    | undefined,
) =>
  JSON.stringify(
    owner
      ? [
          owner.id,
          owner.accountKey,
          owner.epoch,
          owner.inFlight,
          owner.threadId,
          owner.turnId,
        ]
      : null,
  );

export { activeTask } from "../backgroundTaskModel";
const labels: Record<components["schemas"]["BackgroundTaskLabel"], string> = {
  running: "Running",
  starting: "Starting",
  pending: "Pending",
  stopping: "Stopping",
  approval: "Needs approval",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
  interrupted: "Interrupted",
  lost: "Outcome unknown",
  waitingForExit: "Waiting for exit",
  unknown: "Unknown",
};
const kindLabels: Record<
  components["schemas"]["BackgroundTaskKindLabel"],
  string
> = {
  monitor: "Monitor",
  command: "Command",
  backgroundCommand: "Background command",
  tool: "Tool",
};
const taskName = (task: Pick<DisplayBackgroundTask, "display">) =>
  task.display.title;
const duration = (seconds: number) =>
  seconds < 60
    ? `${Math.floor(seconds)}s`
    : seconds < 3600
      ? `${Math.floor(seconds / 60)}m ${Math.floor(seconds % 60)}s`
      : `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
const elapsed = (
  task: Pick<DisplayBackgroundTask, "display" | "created"> &
    Pick<WorkspaceTask, "durationMs" | "finished">,
  now: number,
) => {
  if (task.durationMs != null) return duration(task.durationMs / 1000);
  if (!activeTask(task) && !task.finished) return "";
  return duration(Math.max(0, (task.finished || now) - task.created));
};
export function backgroundTaskState(
  task: Pick<DisplayBackgroundTask, "display">,
): ChatIndicatorKind {
  return task.display.indicator;
}
function TaskStatus({
  task,
}: {
  task: Pick<DisplayBackgroundTask, "display">;
}) {
  return <span className="task-status">{labels[task.display.label]}</span>;
}
const iconFor = (task: Pick<DisplayBackgroundTask, "display">) =>
  task.display.kind === BackgroundTaskKind.monitor
    ? Activity
    : task.display.kind === BackgroundTaskKind.command
      ? Terminal
      : Wrench;
const taskKindLabel = (task: Pick<DisplayBackgroundTask, "display">) =>
  kindLabels[task.display.kindLabel];
export default function BackgroundTasks({
  opened,
  close,
  afterClose,
  data,
  leadId,
  openAgent,
  refresh,
  notify,
  initialFocus,
}: {
  opened: boolean;
  close: () => void;
  afterClose?: (() => void) | undefined;
  data: Snapshot;
  leadId?: string | undefined;
  openAgent: (id: string) => void;
  refresh: () => Promise<void>;
  notify: (s: string) => void;
  initialFocus?: { id: string; leadId: string; requestId: string } | undefined;
}) {
  const { get, post } = useServerApi();
  const services = useServerServices();
  const outbox = sessionOutboxRequests();
  const controlSuffix = `${services?.owner.id || "local"}:${leadId}`;
  const controlStorageKey = outbox.key("processControl", controlSuffix);
  const controlScope = useRef(controlStorageKey);
  controlScope.current = controlStorageKey;
  const [kind, setKind] = useState("all"),
    [query, setQuery] = useState(""),
    [selection, setSelection] = useState<{
      scope?: string | undefined;
      id: string;
      requested?: boolean | undefined;
      source?: "process" | "task" | undefined;
    } | null>(null),
    [mobileDetail, setMobileDetail] = useState(false),
    [now, setNow] = useState(Date.now() / 1000);
  const processes = useSubscription(
    opened && leadId ? { kind: "processes", agentId: leadId } : null,
  );
  const activities = processes.data?.activities ?? [];
  const processRecords = activities.flatMap((entry) =>
    entry.source === "process" ? [entry.value] : [],
  );
  const processLoadError = processes.error ? errorText(processes.error) : "";
  const processLoading = processes.status === "loading";
  const [controlPending, setControlPending] = useState(false);
  const [includeWorkers, setIncludeWorkers] = useState(false);
  const [pendingControl, setPendingControl] =
    useState<PostBody<"/api/processes/action"> | null>(null);
  const [controlResult, setControlResult] =
    useState<PostResult<"/api/processes/action"> | null>(null);
  const appliedFocus = useRef<string | undefined>(undefined);
  const agents = data.threads,
    owner = (id: string) => agents.find((a) => a.id === id),
    scopeOwner = owner(leadId || ""),
    responseTarget = processes.data?.responseTarget;
  useEffect(() => {
    if (!opened) return;
    const timer = setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => clearInterval(timer);
  }, [opened]);
  useEffect(() => {
    if (!leadId) return;
    setControlResult(null);
    setControlPending(false);
    setIncludeWorkers(false);
    try {
      const raw = outbox.get("processControl", controlSuffix);
      setPendingControl(
        raw ? (JSON.parse(raw) as PostBody<"/api/processes/action">) : null,
      );
    } catch {
      setPendingControl(null);
    }
  }, [leadId, controlStorageKey]);
  const processTasks = processRecords;
  const active = activities.length;
  const matchesSearch = (task: DisplayBackgroundTask) =>
    `${task.display.title} ${owner(task.agent)?.name} ${task.cwd ?? ""} ${labels[task.display.label]}`
      .toLowerCase()
      .includes(query.toLowerCase());
  const filtered = activities.filter(
    ({ value }) =>
      (kind === "all" || value.display.kind === kind) && matchesSearch(value),
  );
  const pendingFocus =
    opened &&
    initialFocus?.leadId === leadId &&
    initialFocus?.requestId !== appliedFocus.current
      ? initialFocus
      : undefined;
  const explicitId =
    pendingFocus?.id ??
    (selection && selection.scope === leadId && selection.requested
      ? selection.id
      : undefined);
  const selectedEntry = explicitId
    ? activities.find((entry) => entry.value.id === explicitId)
    : selection && selection.scope === leadId
      ? activities.find((entry) => entry.value.id === selection.id)
      : filtered[0];
  const selectedTask = selectedEntry?.value;
  const selectedProcess =
    selectedEntry?.source === "process" ? selectedEntry.value : undefined;
  const submitControl = async (body: PostBody<"/api/processes/action">) => {
    setControlPending(true);
    try {
      const result = await post("/api/processes/action", body, {
        timeoutMs: 30000,
      });
      if (controlScope.current !== controlStorageKey) return;
      setControlResult(result);
      if (result.status === GeneratedApiProcessActionStatus.uncertain) {
        setPendingControl(body);
        outbox.set("processControl", controlSuffix, JSON.stringify(body));
        notify(
          "The stop result is unknown. Check its receipt before another stop.",
        );
      } else {
        outbox.remove("processControl", controlSuffix);
        setPendingControl(null);
        const outcomes = (result.processes || [])
          .map((item) => item.status)
          .join(", ");
        notify(
          result.status === GeneratedApiProcessActionStatus.partial
            ? `Some processes did not stop: ${outcomes || "see each process result"}`
            : `Stop result: ${outcomes || result.response?.status || "unknown"}`,
        );
      }
      await refresh();
    } catch (error) {
      if (controlScope.current !== controlStorageKey) return;
      if (
        error instanceof ApiError &&
        error.status >= 400 &&
        error.status < 500
      ) {
        outbox.remove("processControl", controlSuffix);
        setPendingControl(null);
        notify(errorText(error));
        return;
      }
      setPendingControl(body);
      outbox.set("processControl", controlSuffix, JSON.stringify(body));
      notify(
        "The stop reply is unknown. Check its receipt before another stop.",
      );
      void error;
    } finally {
      if (controlScope.current === controlStorageKey) setControlPending(false);
    }
  };
  const checkControlReceipt = async (
    body: PostBody<"/api/processes/action">,
  ) => {
    setControlPending(true);
    try {
      const receipt = await get("/api/processes/action", {
        query: {
          requestId: body.requestId,
          agent: body.agent,
          accountKey: body.accountKey,
          epoch: body.epoch,
        },
      });
      if (controlScope.current !== controlStorageKey) return;
      if (
        !receipt.found ||
        receipt.result.status === GeneratedApiProcessActionStatus.uncertain
      ) {
        notify(
          "The stop result is still unknown. Check its receipt again later.",
        );
        return;
      }
      setControlResult(receipt.result);
      outbox.remove("processControl", controlSuffix);
      setPendingControl(null);
      const outcomes = (receipt.result.processes || [])
        .map((item) => item.status)
        .join(", ");
      notify(
        receipt.result.status === GeneratedApiProcessActionStatus.partial
          ? `Some processes did not stop: ${outcomes || "see each process result"}`
          : `Stop result: ${outcomes || receipt.result.response?.status || "unknown"}`,
      );
      await refresh();
    } catch {
      if (controlScope.current !== controlStorageKey) return;
      notify(
        "The stop result is still unknown. Check its receipt again later.",
      );
    } finally {
      if (controlScope.current === controlStorageKey) setControlPending(false);
    }
  };
  const runProcessControl = (
    control:
      | {
          action: Extract<
            PostBody<"/api/processes/action">["action"],
            "stop_selected"
          >;
          process: DisplayProcessTask;
        }
      | {
          action: Extract<
            PostBody<"/api/processes/action">["action"],
            "stop_owned" | "stop_response"
          >;
        },
  ) => {
    if (!leadId || controlPending || processLoading || processLoadError) return;
    if (pendingControl) {
      notify("Check the earlier stop result before starting another stop.");
      return;
    }
    const identity =
      control.action === "stop_response"
        ? responseTarget
        : scopeOwner?.epoch == null
          ? null
          : {
              agent: leadId,
              accountKey: scopeOwner.accountKey || "default",
              epoch: scopeOwner.epoch,
            };
    if (!identity) {
      if (control.action === "stop_response")
        notify("Refresh the process list before stopping this response.");
      return;
    }
    const scope = {
      requestId: randomUUID(),
      agent: identity.agent,
      accountKey: identity.accountKey,
      epoch: identity.epoch,
    };
    let body: PostBody<"/api/processes/action">;
    switch (control.action) {
      case "stop_response":
        if (!responseTarget) {
          notify("Refresh the process list before stopping this response.");
          return;
        }
        body = {
          ...scope,
          action: control.action,
          ...responseStopIdentity(responseTarget),
        };
        break;
      case "stop_selected":
        body = {
          ...scope,
          action: control.action,
          process: processIdentity(control.process),
          ...(control.process.agent !== leadId && includeWorkers
            ? { includeDescendants: true }
            : {}),
        };
        break;
      case "stop_owned":
        body = {
          ...scope,
          action: control.action,
          processes: processTasks
            .filter(
              (task) =>
                task.display.actions.includes(BackgroundTaskAction.stop) &&
                (task.agent === leadId ||
                  (includeWorkers && owner(task.agent)?.rootId === leadId)),
            )
            .map(processIdentity),
          includeDescendants: includeWorkers,
        };
        break;
      default: {
        const exhaustive: never = control;
        return exhaustive;
      }
    }
    outbox.set("processControl", controlSuffix, JSON.stringify(body));
    setPendingControl(body);
    void submitControl(body);
  };
  const selectedId = selectedTask?.id;
  const selectedSource = selectedEntry?.source;
  useEffect(() => {
    if (explicitId && !selectedId) return;
    setSelection((previous) =>
      selectedId
        ? previous &&
          previous.scope === leadId &&
          previous.id === selectedId &&
          previous.source === selectedSource
          ? previous
          : { scope: leadId, id: selectedId, source: selectedSource }
        : null,
    );
  }, [leadId, selectedId, selectedSource, selection, explicitId]);
  useEffect(() => {
    setMobileDetail(false);
  }, [leadId]);
  useEffect(() => {
    if (!opened) setMobileDetail(false);
  }, [opened]);
  useEffect(() => {
    if (!opened || !initialFocus || initialFocus.leadId !== leadId) return;
    appliedFocus.current = initialFocus.requestId;
    setSelection({ scope: leadId, id: initialFocus.id, requested: true });
    setKind("all");
    setQuery("");
    setMobileDetail(true);
  }, [opened, initialFocus?.requestId, leadId]);
  return (
    <Drawer
      opened={opened}
      onClose={close}
      {...(afterClose ? { onExitTransitionEnd: afterClose } : {})}
      returnFocus={!afterClose}
      position="right"
      size={940}
      padding={0}
      title={
        <div className="tasks-title">
          <Activity size={19} />
          <strong>Current activity</strong>
          <Badge variant="light" color="gray" size="sm">
            {active} active
          </Badge>
        </div>
      }
      classNames={{
        content: "tasks-drawer",
        body: "tasks-drawer-body",
        header: "tasks-drawer-header",
      }}
    >
      <div className="tasks-toolbar">
        <div className="tasks-filters">
          <NativeSelect
            aria-label="Task type"
            value={kind}
            onChange={(e) => {
              setKind(e.target.value);
              setSelection(null);
              setMobileDetail(false);
            }}
            data={[
              { value: "all", label: "All" },
              { value: "command", label: "Commands" },
              { value: "monitor", label: "Monitors" },
            ]}
            size="xs"
          />
        </div>
        <span className="tasks-filter-help">
          Commands run in the chat. Monitors run commands in the background.
        </span>
        <div className="tasks-process-actions" aria-label="Process controls">
          <Checkbox
            size="xs"
            label="Include worker processes"
            checked={includeWorkers}
            onChange={(event) => setIncludeWorkers(event.currentTarget.checked)}
            disabled={controlPending || !!pendingControl}
          />
          <Button
            size="xs"
            color="red"
            leftSection={<Square size={12} />}
            loading={controlPending}
            disabled={responseStopDisabled(responseTarget, !!pendingControl)}
            onClick={() => runProcessControl({ action: "stop_response" })}
          >
            Stop response
          </Button>
          <Button
            size="xs"
            color="red"
            variant="light"
            leftSection={<Square size={12} />}
            loading={controlPending}
            disabled={
              !processRecords.some(
                (process) =>
                  process.display.actions.includes(BackgroundTaskAction.stop) &&
                  (includeWorkers || process.agent === leadId),
              ) ||
              !!pendingControl ||
              processLoading ||
              !!processLoadError
            }
            onClick={() => runProcessControl({ action: "stop_owned" })}
          >
            Stop owned processes
          </Button>
          {pendingControl && (
            <Button
              size="xs"
              variant="light"
              loading={controlPending}
              onClick={() => void checkControlReceipt(pendingControl)}
            >
              Check stop result
            </Button>
          )}
        </div>
      </div>
      {processLoadError && (
        <p role="alert" className="task-error">
          Process list unavailable: {processLoadError}
        </p>
      )}
      {processLoading && <p role="status">Loading processes…</p>}
      {controlResult?.response && (
        <p
          role={
            controlResult.status === GeneratedApiProcessActionStatus.uncertain
              ? "alert"
              : "status"
          }
        >
          Response stop: {String(controlResult.response.status)}
          {"error" in controlResult.response && controlResult.response.error
            ? `, ${controlResult.response.error}`
            : ""}
        </p>
      )}
      {!!controlResult?.processes?.length && (
        <ul
          aria-label="Process stop results"
          className="process-control-results"
        >
          {controlResult.processes.map((item, index) => {
            const key = item.key;
            const process = processRecords.find(
              (candidate) => candidate.key === key,
            );
            return (
              <li key={`${key}:${index}`}>
                {process?.name || owner(item.agent)?.name || "Process"}:{" "}
                {item.status}
                {"error" in item && typeof item.error === "string"
                  ? `, ${item.error}`
                  : ""}
              </li>
            );
          })}
        </ul>
      )}
      <div
        className={`tasks-content ${mobileDetail && (selectedTask || explicitId) ? "show-task-detail" : ""}`}
      >
        <section className="tasks-list" aria-label="Task list">
          <div className="tasks-search">
            <TextInput
              size="sm"
              aria-label="Find a task"
              placeholder="Search commands or agents"
              leftSection={<Search size={14} />}
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setSelection(null);
                setMobileDetail(false);
              }}
            />
          </div>
          <div className="tasks-rows">
            {filtered.map(({ source, value: task }) => (
              <BackgroundTaskStrip
                key={`${source}:${task.id}`}
                task={task}
                owner={owner(task.agent)}
                now={now}
                selected={
                  selection?.source === source && selectedTask?.id === task.id
                }
                select={() => {
                  setSelection({ scope: leadId, id: task.id, source });
                  setMobileDetail(true);
                }}
              />
            ))}
            {!filtered.length && (
              <div className="tasks-empty">
                <Check size={24} />
                <strong>
                  {query ? "No matching tasks" : "No active tasks"}
                </strong>
                <p>
                  {query
                    ? "Try another command or agent name."
                    : "Agent commands and tool calls appear here."}
                </p>
              </div>
            )}
          </div>
        </section>
        {selectedTask ? (
          <TaskDetail
            key={selectedTask.id}
            activity={selectedEntry!}
            owner={owner(selectedTask.agent)}
            now={now}
            requests={data.runtime.requests}
            back={() => setMobileDetail(false)}
            openAgent={() => {
              close();
              openAgent(selectedTask.agent);
            }}
            refresh={refresh}
            notify={notify}
            canStop={
              !processLoading &&
              !processLoadError &&
              !pendingControl &&
              !!selectedProcess?.display.actions.includes(
                BackgroundTaskAction.stop,
              ) &&
              (includeWorkers || selectedTask.agent === leadId)
            }
            onStop={() => {
              if (selectedProcess)
                runProcessControl({
                  action: "stop_selected",
                  process: selectedProcess,
                });
            }}
            controlPending={controlPending}
          />
        ) : (
          <div className="task-detail-empty">
            <Terminal size={32} />
            <p role="status">
              {explicitId
                ? "The selected task is no longer active in this chat."
                : "Select a task to inspect its output."}
            </p>
            {explicitId && (
              <Button variant="subtle" onClick={() => setMobileDetail(false)}>
                Back to tasks
              </Button>
            )}
          </div>
        )}
      </div>
    </Drawer>
  );
}

/** Same dispatch-strip geometry as an agent; only actual commands use code type. */
export function BackgroundTaskStrip({
  task,
  owner,
  now,
  selected,
  select,
}: {
  task: DisplayBackgroundTask;
  owner?: Agent | undefined;
  now: number;
  selected: boolean;
  select: () => void;
}) {
  return (
    <UnstyledButton
      data-task={task.id}
      data-studio-state={backgroundTaskState(task)}
      className={`task-row ${selected ? "selected" : ""}`}
      aria-pressed={selected}
      onClick={select}
    >
      <div className="task-row-heading">
        <span className="task-row-kind">{taskKindLabel(task)}</span>
        <span className="task-age">{elapsed(task, now)}</span>
      </div>
      <strong
        className={task.command ? "task-command-title" : ""}
        title={taskName(task)}
      >
        {taskName(task)}
      </strong>
      <div className="task-row-footer">
        <span>{owner?.name || "Agent"}</span>
        <TaskStatus task={task} />
      </div>
    </UnstyledButton>
  );
}

export function TaskDetail({
  activity,
  owner,
  now,
  requests,
  back,
  openAgent,
  refresh,
  notify,
  canStop,
  onStop,
  controlPending,
}: {
  activity: DeepReadonly<components["schemas"]["BackgroundTaskView"]>;
  owner?: Agent | undefined;
  now: number;
  requests: PendingRequest[];
  back: () => void;
  openAgent: () => void;
  refresh: () => Promise<void>;
  notify: (s: string) => void;
  canStop: boolean;
  onStop: () => void;
  controlPending: boolean;
}) {
  const { get, post } = useServerApi();
  const summary = activity.value;
  const process = activity.source === "process" ? activity.value : undefined;

  const [fileOutput, setFileOutput] = useState<{
    text: string;
    truncated: boolean;
  } | null>(null);
  const [filePending, setFilePending] = useState(false);
  const [fileError, setFileError] = useState("");
  const [detail, setDetail] = useState<TaskDetailResponse | null>(null),
    [loadError, setLoadError] = useState("");
  const loadDetail = async () => {
    try {
      const value = await get("/api/task", { query: { id: summary.id } });
      setDetail(value);
      setLoadError("");
    } catch (error) {
      setLoadError(errorText(error));
    }
  };
  const resolvedDetail = detail?.id === summary.id ? detail : null;
  const task = summary;
  const outputSupported = task.display.actions.includes(
    BackgroundTaskAction.readOutput,
  );
  const outputTail = outputSupported
    ? (fileOutput?.text ?? task.tail)
    : undefined;
  const outputError = task.error;
  const [pending, setPending] = useState(false),
    [follow, setFollow] = useState(true),
    [copied, setCopied] = useState(false);
  const output = useRef<HTMLPreElement>(null);
  useEffect(() => {
    if (follow && output.current)
      output.current.scrollTop = output.current.scrollHeight;
  }, [outputTail, follow]);
  const act: BackgroundAction = async <Path extends ApiPostPath>(
    path: Path,
    body: PostInput<Path>,
  ) => {
    setPending(true);
    try {
      const result = await post(path, body);
      if (
        result &&
        typeof result === "object" &&
        "error" in result &&
        result.error
      )
        throw new Error(displayError(result.error));
      void refresh().catch((error) => notify(errorText(error)));
      return true;
    } catch (error) {
      notify(errorText(error));
      return false;
    } finally {
      setPending(false);
    }
  };
  const request = requests.find(
    (r) => r.request.kind === "monitor_start" && r.request.monitor === task.id,
  );
  const Icon = iconFor(task),
    age = elapsed(task, now);
  return (
    <section
      className="task-detail"
      aria-label="Task details"
      data-task-detail={task.id}
    >
      <div className="task-detail-top">
        <Button
          className="task-back"
          size="compact-xs"
          leftSection={<ArrowLeft size={14} />}
          onClick={back}
        >
          Activity
        </Button>
        <span>
          <Icon size={15} />
          {taskKindLabel(task)}
        </span>
        <TaskStatus task={task} />
      </div>
      <div className="task-detail-scroll">
        <h2 className={task.command ? "task-command-title" : ""}>
          {taskName(task)}
        </h2>
        <Button
          size="compact-xs"
          className="task-owner"
          rightSection={<ArrowUpRight size={13} />}
          onClick={openAgent}
        >
          {owner?.name || "Open agent"}
        </Button>
        <div className="task-facts">
          {age && (
            <span>
              <Clock size={12} />
              {age}
            </span>
          )}
          {task.exitCode != null && (
            <span className={task.exitCode !== 0 ? "task-failed" : ""}>
              Exit {task.exitCode}
            </span>
          )}
          {process?.timeout_ms != null && (
            <span>Timeout {duration(process?.timeout_ms / 1000)}</span>
          )}
          {task.processId && <span>Process {task.processId}</span>}
        </div>
        {task.cwd && <p className="task-cwd">{task.cwd}</p>}
        {task.display.kind === BackgroundTaskKind.monitor &&
          activeTask(task) && (
            <p className="task-note">
              {task.display.actions.includes(BackgroundTaskAction.approve)
                ? "The command waits for your approval."
                : "The agent receives the result when this command exits."}
            </p>
          )}
        {process?.display.kind === BackgroundTaskKind.monitor &&
          process.outputWakeBudget && (
            <div className="task-wake-budget" role="status">
              <Text size="sm">
                {process.outputWakeBudget.totalSuppressedMatches} matches
                suppressed
              </Text>
              {process.outputWakeBudget.paused && (
                <>
                  <Text fw={600} size="sm">
                    Output wakes paused
                  </Text>
                  <Text size="sm">
                    {task.display.label === BackgroundTaskLabel.running
                      ? "The command is still running. Its exit will still wake the agent."
                      : "The command exit will still wake the agent."}
                  </Text>
                </>
              )}
              {activeTask(task) && (
                <MonitorRestart
                  task={process}
                  notify={notify}
                  refresh={refresh}
                />
              )}
            </div>
          )}
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
                  setFileOutput(
                    await get("/api/task/output", {
                      query: { id: task.id, agent: task.agent },
                    }),
                  );
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
        {task.display.kind !== BackgroundTaskKind.monitor &&
          !resolvedDetail && (
            <Button
              size="compact-xs"
              variant="subtle"
              onClick={() => void loadDetail()}
            >
              Read tool input
            </Button>
          )}
        {resolvedDetail?.arguments && (
          <details className="task-input">
            <summary>Tool input</summary>
            <pre>{resolvedDetail.arguments}</pre>
          </details>
        )}
        {loadError && (
          <p role="alert" className="task-error">
            Output unavailable: {loadError}
          </p>
        )}
        {outputError && (
          <p role="alert" className="task-error">
            <ErrorDescription value={outputError} />
          </p>
        )}
        {process?.stdinError && (
          <p role="alert" className="task-error">
            <ErrorDescription value={process?.stdinError} />
          </p>
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
                  size="compact-xs"
                  aria-pressed={follow}
                  onClick={() => setFollow(!follow)}
                  leftSection={<ChevronDown size={12} />}
                  color={follow ? "signal" : "gray"}
                >
                  Follow
                </Button>
                <ActionIcon
                  size="sm"
                  aria-label="Download task log"
                  disabled={pending || (!!task.outputFile && !fileOutput)}
                  onClick={() =>
                    void downloadLog(
                      task,
                      resolvedDetail,
                      notify,
                      fileOutput?.text,
                    )
                  }
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
            (process?.bytes || 0) >
              new TextEncoder().encode(outputTail || "").length) && (
            <p className="task-output-limit">
              Latest output shown
              {process?.log ? ". The saved log is available below." : "."}
            </p>
          )}
        </div>
        {(task.display.kind === BackgroundTaskKind.monitor ||
          task.display.actions.includes(BackgroundTaskAction.input)) && (
          <ProcessInput
            task={task}
            owner={owner}
            pending={pending}
            act={act}
            receiptOnly={
              !task.display.actions.includes(BackgroundTaskAction.input) &&
              !task.display.actions.includes(BackgroundTaskAction.resize)
            }
          />
        )}
        <dl className="task-record">
          <div>
            <dt>Created</dt>
            <dd>{localDateTime(new Date(task.created * 1000))}</dd>
          </div>
          {task.finished && (
            <div>
              <dt>Finished</dt>
              <dd>{localDateTime(new Date(task.finished * 1000))}</dd>
            </div>
          )}
          {process?.log && (
            <div>
              <dt>Saved log</dt>
              <dd>{process?.log}</dd>
            </div>
          )}
        </dl>
      </div>
      <div className="task-actions">
        {request &&
          task.display.actions.includes(BackgroundTaskAction.approve) && (
            <Button
              size="xs"
              variant="light"
              loading={pending}
              onClick={() =>
                void act("/api/answer", { id: request.id, decision: "accept" })
              }
            >
              Approve command
            </Button>
          )}
        {canStop && (
          <Button
            size="xs"
            color="red"
            leftSection={<Square size={12} />}
            loading={controlPending}
            disabled={controlPending}
            onClick={onStop}
          >
            Stop selected process
          </Button>
        )}
      </div>
    </section>
  );
}

export function ProcessInput({
  task,
  owner,
  pending,
  act,
  receiptOnly = false,
}: {
  task: Pick<DisplayBackgroundTask, "id" | "display">;
  owner?: Agent | undefined;
  pending: boolean;
  act: BackgroundAction;
  receiptOnly?: boolean;
}) {
  const api = useServerApi();
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
  const native = task.display.kind === BackgroundTaskKind.command;
  const checkReceipt = async () => {
    if (!owner || owner.epoch == null || checkingReceipt || pending) return;
    setCheckingReceipt(true);
    try {
      const status = await api.checkMonitorInputReceipt(task.id, {
        agent: owner.id,
        accountKey: owner.accountKey || "default",
        epoch: owner.epoch,
      });
      setReceiptState(
        status === "acknowledged"
          ? "The process acknowledged the input."
          : status === "rejected"
            ? "The process rejected the input."
            : status === "none"
              ? "No input request needs a receipt check."
              : "The input result is unknown. Do not send different input.",
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
            disabled={pending || checkingReceipt || stdinUnavailable}
            maxLength={16000}
          />
          <Button
            size="sm"
            type="submit"
            disabled={pending || checkingReceipt || stdinUnavailable}
          >
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
              disabled={pending || checkingReceipt || stdinUnavailable}
              onClick={() =>
                void act("/api/monitor/input", { id: task.id, text: "\u0003" })
              }
            >
              Ctrl+C
            </Button>
            <Button
              size="compact-xs"
              disabled={pending || checkingReceipt || stdinUnavailable}
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
                <NumberInput
                  label="Rows"
                  size="xs"
                  value={rows}
                  onChange={setRows}
                  min={1}
                  max={500}
                  allowDecimal={false}
                />
                <NumberInput
                  label="Columns"
                  size="xs"
                  value={cols}
                  onChange={setCols}
                  min={1}
                  max={500}
                  allowDecimal={false}
                />
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
}

async function downloadLog(
  task: Pick<DisplayBackgroundTask, "id" | "display" | "outputTruncated"> &
    Pick<BackgroundTask, "tail">,
  detail: TaskDetailResponse | null,
  notify: (s: string) => void,
  fileText?: string,
) {
  try {
    let blob: Blob, name: string;
    if (task.display.kind === BackgroundTaskKind.monitor) {
      const result = await apiDownload("/api/monitor/log", { id: task.id });
      blob = result.blob;
      name = result.name;
      if (result.truncated)
        notify("The download contains the retained part of the log.");
    } else {
      blob = new Blob([fileText ?? detail?.tail ?? task.tail ?? ""], {
        type: "text/plain",
      });
      name = `command-${task.id}.log`;
      if (task.outputTruncated)
        notify("The download contains the retained output.");
    }
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = name;
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 10000);
  } catch (error) {
    notify(errorText(error));
  }
}
