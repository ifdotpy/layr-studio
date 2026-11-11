import { useState } from "react";
import { textField } from "../../../../stores/fields.ts";
import { WorkStatus } from "../../../../generated/api.ts";
import {
  Badge,
  Button,
  NativeSelect,
  Textarea,
  TextInput,
} from "@mantine/core";
import { observer, Observer } from "mobx-react-lite";
import { createArea } from "../../../../app/area.ts";
import {
  useServerApi,
  useServerStorage,
  useServerStore,
} from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  gateFields,
  parseGate,
  prettyJson,
  taskDraft,
  taskBody,
  taskSchemaError,
  type Gate,
} from "./taskReviewModel.ts";

import { Owned } from "../../../ui/Owned/Owned.tsx";
import { createTasksStore, type TasksStore, type Task } from "./TasksStore.ts";
type TaskProps = { task: Task };
const [TaskProvider, useTasks] = createArea<TasksStore>("Tasks");

const JsonView = observer(
  ({ value, label }: { value: unknown; label: string }) => (
    <pre aria-label={label} className="task-review-json">
      {prettyJson(value)}
    </pre>
  ),
);

const GateEditor = observer(
  ({
    value,
    onChange,
  }: {
    value: ReturnType<typeof gateFields>;
    onChange(value: ReturnType<typeof gateFields>): void;
  }) => (
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
  ),
);

const DefaultGate = observer(({ gate }: { gate: Gate }) => {
  const area = useTasks();
  const [gateInput, setGateInput] = useState(() => gateFields(gate));
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
        onSubmit={(event) => {
          event.preventDefault();
          void area.write(() => ({
            action: "set_default_gate",
            gate: parseGate(gateInput.command, gateInput.timeout),
          }));
        }}
      >
        <fieldset disabled={area.busy}>
          <GateEditor value={gateInput} onChange={setGateInput} />
          {area.error && <p role="alert">{area.error}</p>}
          <Button type="submit">Save team default gate</Button>
        </fieldset>
      </form>
    </details>
  );
});

const TaskEditor = observer(({ task }: { task?: Task | undefined }) => {
  const area = useTasks();
  const [s, setDraft] = useState(() => taskDraft(task));
  const patch = (value: Partial<typeof s>) =>
    setDraft((current) => ({ ...current, ...value }));
  const schemaError = taskSchemaError(s.schema);
  return (
    <form
      className="task-review-card"
      aria-label={task ? "Update task" : "Create task"}
      onSubmit={(event) => {
        event.preventDefault();
        const values = new FormData(event.currentTarget);
        void area.write(() => taskBody(s, task, values));
      }}
    >
      <h3>{task ? "Update task" : "Create task"}</h3>
      <fieldset disabled={area.busy}>
        <TextInput
          label="Title"
          required
          value={s.title}
          onChange={(e) => patch({ title: e.currentTarget.value })}
        />
        <Textarea
          label="Description"

          name="description"
          defaultValue={s.description}
          autosize
          minRows={3}
        />
        <TextInput label="Owner agent ID" name="owner" defaultValue={s.owner} />
        <NativeSelect label="Task status" name="status" defaultValue={s.status}>
          {task && <option value="">Keep {task.status}</option>}
          <option value="ready">ready</option>
          <option value="blocked">blocked</option>
        </NativeSelect>
        <label className="task-review-dependencies">
          Dependencies
          <select
            multiple
            aria-label="Dependencies"
            name="dependencies"
            defaultValue={s.dependencies}
          >
            {area.tasks
              ?.filter((candidate) => candidate !== task)
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
            value={s.mode}
            onChange={(e) => patch({ mode: e.currentTarget.value })}
            data={[
              { value: "inherit", label: "Use team default" },
              { value: "custom", label: "Set task gate" },
            ]}
          />
        )}
        {s.mode === "custom" && (
          <GateEditor
            value={s.gateInput}
            onChange={(gateInput) => patch({ gateInput })}
          />
        )}
        <Textarea
          label="Result schema (JSON)"
          description="The server validates the schema before it saves the task. Use true to allow any JSON result."
          error={schemaError || undefined}

          value={s.schema}
          onChange={(e) => patch({ schema: e.currentTarget.value })}
          autosize
          minRows={5}
        />
        {area.error && <p role="alert">{area.error}</p>}
        <div className="task-review-actions">
          <Button type="submit" disabled={!s.title.trim() || !!schemaError}>
            Save task
          </Button>
          <Button variant="default" onClick={() => area.edit(null)}>
            Cancel edit
          </Button>
        </div>
      </fieldset>
    </form>
  );
});

const TaskDetail = observer(function TaskDetail({ task }: TaskProps) {
  return (
    <article className="task-review-card" aria-label="Task detail">
      <TaskHeader task={task} />
      <TaskFacts task={task} />
      <TaskGate task={task} />
      <TaskEvidence task={task} />
      <ReviewControls task={task} />
      <TaskHistory task={task} />
    </article>
  );
});
const TaskHeader = observer(function TaskHeader({ task }: TaskProps) {
  const area = useTasks();
  return (
    <>
      <div className="task-review-actions">
        <h3>{task.title}</h3>
        <Badge>{task.policy.displayStatus}</Badge>
        <Button
          variant="default"
          disabled={
            area.busy ||
            task.status === WorkStatus.review ||
            task.status === WorkStatus.accepted
          }
          onClick={() => area.edit("update")}
        >
          Update task
        </Button>
      </div>
      {task.status === WorkStatus.review && (
        <p>Reject the result before you edit this task.</p>
      )}
      {task.status === WorkStatus.accepted && (
        <p>Create a follow-up task to change accepted work.</p>
      )}
    </>
  );
});
const TaskFacts = observer(function TaskFacts({ task }: TaskProps) {
  return (
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
  );
});
const ReviewControls = observer(function ReviewControls({ task }: TaskProps) {
  const area = useTasks(),
    s = area;
  const [reason, setReason] = useState(""),
    [overrideReason, setOverrideReason] = useState("");
  return (
    <>
      {task.status === WorkStatus.review && (
        <fieldset disabled={area.busy}>
          <legend>Review decision</legend>
          <Textarea
            label="Decision reason"
            required
            value={reason}
            onChange={(e) => setReason(e.currentTarget.value)}
          />
          {task.policy.gateOverrideRequired && (
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
                s.pendingDecision?.action === "reject" ||
                (!s.pendingDecision &&
                  (!reason.trim() ||
                    (task.policy.gateOverrideRequired &&
                      !overrideReason.trim()))) ||
                (!s.pendingDecision && !task.policy.acceptAvailable)
              }
              onClick={() =>
                void s.decide(task, "accept", reason, overrideReason)
              }
            >
              {task.policy.gateOverrideRequired
                ? "Accept with override"
                : "Accept task"}
            </Button>
            <Button
              color="red"
              variant="outline"
              disabled={
                s.pendingDecision?.action === "accept" ||
                (!s.pendingDecision && !reason.trim())
              }
              onClick={() =>
                void s.decide(task, "reject", reason, overrideReason)
              }
            >
              Reject task
            </Button>
          </div>
          {!task.policy.acceptAvailable && (
            <p>Acceptance is unavailable until a result is ready for review.</p>
          )}
        </fieldset>
      )}
    </>
  );
});
const TaskHistory = observer(function TaskHistory({ task }: TaskProps) {
  return (
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
        <summary>Decision history ({task.decisions?.length || 0})</summary>
        {task.decisions?.map((decision, index) => (
          <JsonView
            key={index}
            label={`Decision ${index + 1}`}
            value={decision}
          />
        ))}
      </details>
    </>
  );
});
const TaskGate = observer(function TaskGate({ task }: TaskProps) {
  const s = useTasks();
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
              onClick={() => void s.downloadLog(gate.monitorId!)}
            >
              Download gate log
            </Button>
          )}
          {s.download.error && <p role="alert">{s.download.error}</p>}
        </>
      )}
    </>
  );
});
const TaskEvidence = observer(({ task }: TaskProps) => {
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
const TaskRow = observer(function TaskRow({ task }: TaskProps) {
  const area = useTasks();
  return (
    <li>
      <button
        disabled={area.busy}
        aria-pressed={area.selected === task}
        onClick={() => area.select(task)}
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
export default function Tasks({
  agentId,
  rootId = agentId,
}: {
  agentId: string;
  rootId?: string;
}) {
  const api = useServerApi(),
    storage = useServerStorage(),
    store = useServerStore();
  return (
    <Owned
      key={rootId}
      create={() => createTasksStore(rootId, api, storage, store)}
    >
      {(area) => (
        <TaskProvider value={area}>
          <section className="task-review" aria-label="Tasks">
            <div className="task-review-actions">
              <Button
                disabled={area.busy || !area.ready}
                onClick={() => area.edit("create")}
              >
                Create task
              </Button>
            </div>
            {area.failure && <p role="alert">{area.failure}</p>}
            {area.notice && <p role="status">{area.notice}</p>}
            {!area.ready && <p>Load tasks...</p>}
            {area.ready && (
              <DefaultGate key={prettyJson(area.gate)} gate={area.gate} />
            )}
            <div className="task-review-filters">
              <NativeSelect
                label="Filter by status"
                {...textField(area, "status")}
                data={[
                  { value: "", label: "All statuses" },
                  ...Object.values(WorkStatus),
                ]}
              />
              <NativeSelect
                label="Filter by owner"
                {...textField(area, "owner")}
                data={[
                  { value: "", label: "All owners" },
                  { value: "unassigned", label: "Unassigned" },
                  ...area.owners,
                ]}
              />
            </div>
            <ul className="task-review-list">
              {area.visibleTasks?.map((task) => (
                <TaskRow key={task.id} task={task} />
              ))}
            </ul>
            {area.tasks && !area.visibleTasks?.length && (
              <p>{area.emptyMessage}</p>
            )}
            {area.editing === "create" ? (
              <TaskEditor key="create" />
            ) : area.task ? (
              area.editing ? (
                <TaskEditor key={area.task.id} task={area.task} />
              ) : (
                <TaskDetail key={area.task.id} task={area.task} />
              )
            ) : null}
          </section>
        </TaskProvider>
      )}
    </Owned>
  );
}
