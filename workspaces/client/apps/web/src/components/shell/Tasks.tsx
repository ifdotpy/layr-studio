import { WorkStatus as GeneratedApiWorkStatus } from "../../generated/api.ts";
import {
  Badge,
  Button,
  NativeSelect,
  Textarea,
  TextInput,
} from "@mantine/core";
import { useRef, useState } from "react";
import { observer, Observer } from "mobx-react-lite";
import {
  agentId as workAgentId,
  type WorkSubscriptionData,
} from "../../generated/observableResources.ts";
import { resourceData } from "../../store/runtime.ts";
import { createArea } from "../area.ts";
import { ApiError, errorText, type PostBody } from "../../api";
import {
  useServerApi,
  useServerStorage,
  useServerStore,
} from "../../servers/ServerServicesContext";
import {
  serverMutationResponseIsUncertain,
  createMutationJournal,
} from "../../servers/mutationIdentity";
import {
  gateFields,
  parseGate,
  parseResultSchema,
  prettyJson,
  type Gate,
} from "./taskReviewModel";
import "./Tasks.css";

type WorkBody = PostBody<"/api/work">;
type Task = WorkSubscriptionData["items"][number];
const [TaskProvider, useTasks] =
  createArea<ReturnType<typeof useTaskActions>>("Tasks");

const JsonView = observer(
  ({ value, label }: { value: unknown; label: string }) => (
    <pre aria-label={label} className="task-review-json">
      {prettyJson(value)}
    </pre>
  ),
);

function GateEditor({
  value,
  onChange,
}: {
  value: ReturnType<typeof gateFields>;
  onChange: (value: ReturnType<typeof gateFields>) => void;
}) {
  return (
    <>
      <Textarea
        label="Gate command"
        description="Leave empty to disable the gate."
        value={value.command}
        onChange={(e) => onChange({ ...value, command: e.currentTarget.value })}
        autosize
        minRows={2}
      />
      <TextInput
        label="Gate timeout (milliseconds)"
        inputMode="numeric"
        disabled={!value.command.trim()}
        value={value.timeout}
        onChange={(e) => onChange({ ...value, timeout: e.currentTarget.value })}
      />
    </>
  );
}

const DefaultGate = observer(({ gate }: { gate: Gate }) => {
  const { write, busy } = useTasks();
  const [gateInput, setGateInput] = useState(() => gateFields(gate));
  const [error, setError] = useState("");
  return (
    <details className="task-review-card">
      <summary>Team default gate</summary>
      <p>The default applies to new tasks. Existing tasks keep their gate.</p>
      {gate ? (
        <JsonView label="Current team default gate" value={gate} />
      ) : (
        <p>No default gate.</p>
      )}
      <form
        onSubmit={async (event) => {
          event.preventDefault();
          setError("");
          try {
            await write({
              action: "set_default_gate",
              gate: parseGate(gateInput.command, gateInput.timeout),
            });
          } catch (failure) {
            setError(errorText(failure));
          }
        }}
      >
        <fieldset disabled={busy}>
          <GateEditor value={gateInput} onChange={setGateInput} />
          {error && <p role="alert">{error}</p>}
          <Button type="submit">Save team default gate</Button>
        </fieldset>
      </form>
    </details>
  );
});

const TaskEditor = observer(({ task }: { task?: Task | undefined }) => {
  const { work, write, busy, setEditing: onClose } = useTasks();
  const [title, setTitle] = useState(task?.title || "");
  const [description, setDescription] = useState(task?.description || "");
  const [owner, setOwner] = useState(task?.owner || "");
  const [dependencies, setDependencies] = useState(() => [
    ...(task?.dependencies || []),
  ]);
  const [status, setStatus] = useState<"ready" | "blocked" | "">(
    task ? "" : "ready",
  );
  const [mode, setMode] = useState(task ? "custom" : "inherit");
  const [gateInput, setGateInput] = useState(() => gateFields(task?.gate));
  const [schema, setSchema] = useState(
    task?.resultSchema === undefined ? "" : prettyJson(task.resultSchema),
  );
  const [error, setError] = useState("");
  let schemaError = "";
  try {
    parseResultSchema(schema);
  } catch (failure) {
    schemaError = errorText(failure);
  }
  return (
    <form
      className="task-review-card"
      aria-label={task ? "Update task" : "Create task"}
      onSubmit={async (event) => {
        event.preventDefault();
        setError("");
        try {
          const resultSchema = parseResultSchema(schema);
          if (task?.resultSchema !== undefined && resultSchema === undefined)
            throw new Error(
              "Use true to allow any JSON result. An existing result schema cannot be removed.",
            );
          const fields = {
            title: title.trim(),
            description,
            owner: owner.trim() || null,
            dependencies,
            ...(status ? { status } : {}),
            ...(mode === "custom"
              ? { gate: parseGate(gateInput.command, gateInput.timeout) }
              : {}),
            ...(resultSchema === undefined
              ? {}
              : { result_schema: resultSchema }),
          };
          const body: WorkBody = task
            ? {
                action: "update",
                task_id: task.id,
                version: task.version,
                ...fields,
              }
            : { action: "create", ...fields };
          if ((await write(body)) === true) onClose(null);
        } catch (failure) {
          setError(errorText(failure));
        }
      }}
    >
      <h3>{task ? "Update task" : "Create task"}</h3>
      <fieldset disabled={busy}>
        <TextInput
          label="Title"
          required
          value={title}
          onChange={(e) => setTitle(e.currentTarget.value)}
        />
        <Textarea
          label="Description"
          autosize
          minRows={3}
          value={description}
          onChange={(e) => setDescription(e.currentTarget.value)}
        />
        <TextInput
          label="Owner agent ID"
          value={owner}
          onChange={(e) => setOwner(e.currentTarget.value)}
        />
        <NativeSelect
          label="Task status"
          value={status}
          onChange={(e) => setStatus(e.currentTarget.value as typeof status)}
        >
          {task && <option value="">Keep {task.status}</option>}
          <option value="ready">ready</option>
          <option value="blocked">blocked</option>
        </NativeSelect>
        <label className="task-review-dependencies">
          Dependencies
          <select
            multiple
            aria-label="Dependencies"
            value={dependencies}
            onChange={(e) =>
              setDependencies(
                Array.from(
                  e.currentTarget.selectedOptions,
                  (option) => option.value,
                ),
              )
            }
          >
            {resourceData(work.current)
              ?.items.filter((candidate) => candidate !== task)
              .map((candidate) => (
                <Observer key={candidate.id}>
                  {() => (
                    <option value={candidate.id}>
                      {candidate.title} ({candidate.policy.displayStatus})
                    </option>
                  )}
                </Observer>
              ))}
          </select>
        </label>
        {!task && (
          <NativeSelect
            label="Task gate"
            value={mode}
            onChange={(e) => setMode(e.currentTarget.value)}
            data={[
              { value: "inherit", label: "Use team default" },
              { value: "custom", label: "Set task gate" },
            ]}
          />
        )}
        {mode === "custom" && (
          <GateEditor value={gateInput} onChange={setGateInput} />
        )}
        <Textarea
          label="Result schema (JSON)"
          description="The server validates the schema before it saves the task. Use true to allow any JSON result."
          autosize
          minRows={5}
          value={schema}
          error={schemaError || undefined}
          onChange={(e) => setSchema(e.currentTarget.value)}
        />
        {error && <p role="alert">{error}</p>}
        <div className="task-review-actions">
          <Button type="submit" disabled={!title.trim() || !!schemaError}>
            Save task
          </Button>
          <Button variant="default" onClick={() => onClose(null)}>
            Cancel edit
          </Button>
        </div>
      </fieldset>
    </form>
  );
});

const TaskDetail = observer(function TaskDetail({ task }: { task: Task }) {
  const { write, busy, setEditing } = useTasks();
  const [reason, setReason] = useState("");
  const [overrideReason, setOverrideReason] = useState("");
  const override = task.policy.gateOverrideRequired;
  const pendingDecision = useRef<WorkBody | null>(null);
  const decide = async (action: "accept" | "reject") => {
    if (
      !pendingDecision.current &&
      (!reason.trim() ||
        (action === "accept" && override && !overrideReason.trim()))
    )
      return;
    const payload = (pendingDecision.current ??= {
      action,
      task_id: task.id,
      version: task.version,
      result: reason.trim(),
      ...(action === "accept" && override
        ? { override_reason: overrideReason.trim() }
        : {}),
    });
    if ((await write(payload)) !== false) pendingDecision.current = null;
  };
  return (
    <article className="task-review-card" aria-label="Task detail">
      <Observer>
        {() => (
          <>
            <div className="task-review-actions">
              <h3>{task.title}</h3>
              <Badge>{task.policy.displayStatus}</Badge>
              <Button
                variant="default"
                disabled={
                  busy ||
                  task.status === GeneratedApiWorkStatus.review ||
                  task.status === GeneratedApiWorkStatus.accepted
                }
                onClick={() => setEditing("update")}
              >
                Update task
              </Button>
            </div>
            {task.status === GeneratedApiWorkStatus.review && (
              <p>Reject the result before you edit this task.</p>
            )}
            {task.status === GeneratedApiWorkStatus.accepted && (
              <p>Create a follow-up task to change accepted work.</p>
            )}
          </>
        )}
      </Observer>
      <Observer>
        {() => (
          <>
            <p className="task-review-description">
              {task.description || "No description."}
            </p>
            <dl>
              <dt>Task ID</dt>
              <dd>{task.id}</dd>
              <dt>Owner</dt>
              <dd>{task.owner || "Unassigned"}</dd>
              <dt>Dependencies</dt>
              <dd>{task.dependencies?.join(", ") || "None"}</dd>
              <dt>Blocked by</dt>
              <dd>{task.policy.blockedBy.join(", ") || "None"}</dd>
              <dt>Version</dt>
              <dd>{task.version}</dd>
              <dt>Revision</dt>
              <dd>{task.results?.at(-1)?.revision || "No result."}</dd>
            </dl>
          </>
        )}
      </Observer>
      <TaskGate task={task} />
      <TaskEvidence task={task} />
      {task.status === GeneratedApiWorkStatus.review && (
        <fieldset disabled={busy}>
          <legend>Review decision</legend>
          <Textarea
            label="Decision reason"
            required
            value={reason}
            onChange={(e) => setReason(e.currentTarget.value)}
          />
          {override && (
            <>
              <p>
                The gate has not passed for this configuration. Acceptance
                requires an override reason.
              </p>
              <Textarea
                label="Gate override reason"
                required
                value={overrideReason}
                onChange={(e) => setOverrideReason(e.currentTarget.value)}
              />
            </>
          )}
          <div className="task-review-actions">
            <Button
              disabled={
                pendingDecision.current?.action === "reject" ||
                (!pendingDecision.current &&
                  (!reason.trim() || (override && !overrideReason.trim()))) ||
                (!pendingDecision.current && !task.policy.acceptAvailable)
              }
              onClick={() => void decide("accept")}
            >
              {override ? "Accept with override" : "Accept task"}
            </Button>
            <Button
              color="red"
              variant="outline"
              disabled={
                pendingDecision.current?.action === "accept" ||
                (!pendingDecision.current && !reason.trim())
              }
              onClick={() => void decide("reject")}
            >
              Reject task
            </Button>
          </div>
          {!task.policy.acceptAvailable && (
            <p>Acceptance is unavailable until a result is ready for review.</p>
          )}
        </fieldset>
      )}
      <Observer>
        {() => (
          <>
            <details>
              <summary>Result history ({task.results?.length || 0})</summary>
              {task.results?.map((result) => (
                <section key={result.id}>
                  <h4>
                    {result.revision} by {result.agent}
                  </h4>
                  <JsonView label={`Result ${result.id}`} value={result} />
                </section>
              ))}
            </details>
            <details>
              <summary>
                Decision history ({task.decisions?.length || 0})
              </summary>
              {task.decisions?.map((decision, index) => (
                <JsonView
                  key={index}
                  label={`Decision ${index + 1}`}
                  value={decision}
                />
              ))}
            </details>
          </>
        )}
      </Observer>
    </article>
  );
});
const TaskGate = observer(function TaskGate({ task }: { task: Task }) {
  const { apiDownload } = useTasks();
  const [logError, setLogError] = useState("");
  const gate = task.results?.at(-1)?.gate;
  return (
    <>
      <h4>Acceptance gate</h4>
      {task.gate ? (
        <JsonView label="Task gate configuration" value={task.gate} />
      ) : (
        <p>No gate configured.</p>
      )}
      <p>
        Gate status: <strong>{gate?.status || "not_run"}</strong>
      </p>
      {gate && (
        <>
          <dl>
            <dt>Exit code</dt>
            <dd>{gate.exitCode ?? "Unknown"}</dd>
            <dt>Gate revision</dt>
            <dd>{gate.revision || "Unknown"}</dd>
          </dl>
          {gate.error && <p role="alert">{gate.error}</p>}
          <pre className="task-review-json" aria-label="Gate output">
            {gate.output || "No output."}
          </pre>
          {gate.log && <p>Saved log: {gate.log}</p>}
          {gate.monitorId && (
            <Button
              variant="subtle"
              onClick={async () => {
                setLogError("");
                try {
                  const result = await apiDownload("/api/monitor/log", {
                    id: gate.monitorId!,
                  });
                  const url = URL.createObjectURL(result.blob);
                  const link = document.createElement("a");
                  link.href = url;
                  link.download = result.name;
                  link.click();
                  setTimeout(() => URL.revokeObjectURL(url), 10000);
                  if (result.truncated)
                    setLogError(
                      "The download contains the retained part of the log.",
                    );
                } catch (failure) {
                  setLogError(errorText(failure));
                }
              }}
            >
              Download gate log
            </Button>
          )}
          {logError && <p role="alert">{logError}</p>}
        </>
      )}
    </>
  );
});
const TaskEvidence = observer(({ task }: { task: Task }) => {
  const latest = task.results?.at(-1);
  const refusal = task.submissionRefusal;
  return (
    <>
      <h4>Result schema</h4>
      {task.resultSchema === undefined ? (
        <p>No result schema.</p>
      ) : (
        <JsonView label="Result schema" value={task.resultSchema} />
      )}
      {refusal && (
        <section aria-label="Submission refusals">
          <h4>Submission refusals</h4>
          <p>
            {refusal.failedSubmits} of {refusal.limit} attempts refused.
          </p>
          {!refusal.retryAllowed && (
            <p role="alert">
              The submission limit is reached. Update the task to allow another
              attempt.
            </p>
          )}
          <ul>
            {refusal.errors.map((error, index) => (
              <li key={index}>
                <code>{error.path || "/"}</code>: {error.message}
              </li>
            ))}
          </ul>
        </section>
      )}
      <h4>Latest result</h4>
      {latest ? (
        <>
          <JsonView
            label="Latest result JSON"
            value={latest.result === undefined ? latest.text : latest.result}
          />
          <p>Checks: {latest.checks}</p>
        </>
      ) : (
        <p>No result submitted.</p>
      )}
    </>
  );
});
const TaskRow = observer(function TaskRow({ task }: { task: Task }) {
  const { selected, status, owner, busy, setSelected, setEditing, setError } =
    useTasks();
  if (
    (status && task.policy.displayStatus !== status) ||
    (owner && (task.owner || "unassigned") !== owner)
  )
    return null;
  return (
    <li>
      <button
        disabled={busy}
        aria-pressed={selected === task}
        onClick={() => {
          setSelected(task);
          setEditing(null);
          setError("");
        }}
      >
        <strong>{task.title}</strong>
        <span>
          {task.policy.displayStatus} · {task.owner || "Unassigned"}
        </span>
        <span>Dependencies: {task.dependencies?.join(", ") || "None"}</span>
      </button>
    </li>
  );
});
function useTaskActions(rootId: string) {
  const { post, apiDownload } = useServerApi();
  const storage = useServerStorage();
  const store = useServerStore();
  const work = store.work(workAgentId(rootId)),
    root = store.agentDetail(workAgentId(rootId));
  const [selected, setSelected] = useState<Task>();
  const [status, setStatus] = useState("");
  const [owner, setOwner] = useState("");
  const [editing, setEditing] = useState<"create" | "update" | null>(null);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const write = async (body: WorkBody) => {
    if (
      busyRef.current ||
      !resourceData(work.current) ||
      !resourceData(root.current)
    )
      return false;
    busyRef.current = true;
    setBusy(true);
    setError("");
    setNotice("");
    let finish: (() => void) | undefined;
    try {
      const payload = { ...body, agent: rootId };
      const identity = await createMutationJournal("task-review", storage)(
        "/api/work",
        payload,
      );
      finish = identity!.finish;
      await post("/api/work", { ...payload, id: identity!.requestId });
      identity!.finish();
      setNotice("The task action completed.");
      return true;
    } catch (failure) {
      setError(errorText(failure));
      if (
        failure instanceof ApiError &&
        failure.status >= 400 &&
        failure.status < 500 &&
        !serverMutationResponseIsUncertain(failure.status, failure.response)
      ) {
        finish?.();
        return "rejected" as const;
      }
      return false;
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };
  return {
    work,
    root,
    selected,
    setSelected,
    status,
    setStatus,
    owner,
    setOwner,
    editing,
    setEditing,
    busy,
    error,
    setError,
    notice,
    write,
    apiDownload,
  };
}
export default function Tasks({
  agentId,
  rootId = agentId,
}: {
  agentId: string;
  rootId?: string;
}) {
  const area = useTaskActions(rootId);
  return (
    <TaskProvider value={area}>
      <section className="task-review" aria-label="Tasks">
        <Observer>
          {() => {
            const workState = area.work.current,
              rootState = area.root.current;
            const ready =
              !!resourceData(workState) && !!resourceData(rootState);
            const failure =
              ("error" in workState && workState.error?.message) ||
              ("error" in rootState && rootState.error?.message) ||
              area.error;
            const gate = resourceData(rootState)?.taskDefaultGate ?? null;
            return (
              <>
                <div className="task-review-actions">
                  <Button
                    disabled={area.busy || !ready}
                    onClick={() => {
                      area.setEditing("create");
                      area.setError("");
                    }}
                  >
                    Create task
                  </Button>
                </div>
                {failure && <p role="alert">{failure}</p>}
                {area.notice && <p role="status">{area.notice}</p>}
                {!ready && <p>Load tasks...</p>}
                {ready && <DefaultGate key={prettyJson(gate)} gate={gate} />}
              </>
            );
          }}
        </Observer>
        <div className="task-review-filters">
          <NativeSelect
            label="Filter by status"
            value={area.status}
            onChange={(e) => area.setStatus(e.currentTarget.value)}
          >
            <option value="">All statuses</option>
            {Object.values(GeneratedApiWorkStatus).map((value) => (
              <option key={value}>{value}</option>
            ))}
          </NativeSelect>
          <Observer>
            {() => (
              <NativeSelect
                label="Filter by owner"
                value={area.owner}
                onChange={(e) => area.setOwner(e.currentTarget.value)}
              >
                <option value="">All owners</option>
                <option value="unassigned">Unassigned</option>
                {Array.from(
                  new Set(
                    resourceData(area.work.current)?.items.flatMap((task) =>
                      task.owner ? [task.owner] : [],
                    ) || [],
                  ),
                ).map((value) => (
                  <option key={value}>{value}</option>
                ))}
              </NativeSelect>
            )}
          </Observer>
        </div>
        <Observer>
          {() => {
            const tasks = resourceData(area.work.current)?.items;
            const visible = tasks?.some(
              (task) =>
                (!area.status || task.policy.displayStatus === area.status) &&
                (!area.owner || (task.owner || "unassigned") === area.owner),
            );
            return (
              <>
                <ul className="task-review-list">
                  {tasks?.map((task) => (
                    <TaskRow key={task.id} task={task} />
                  ))}
                </ul>
                {tasks && !visible && (
                  <p>
                    {tasks.length
                      ? "No tasks match these filters."
                      : "No tasks."}
                  </p>
                )}
              </>
            );
          }}
        </Observer>
        <Observer>
          {() => {
            const task = resourceData(area.work.current)?.items.find(
              (item) => item === area.selected,
            );
            if (area.editing === "create") return <TaskEditor key="create" />;
            return task ? (
              area.editing ? (
                <TaskEditor key={task.id} task={task} />
              ) : (
                <TaskDetail key={task.id} task={task} />
              )
            ) : null;
          }}
        </Observer>
      </section>
    </TaskProvider>
  );
}
