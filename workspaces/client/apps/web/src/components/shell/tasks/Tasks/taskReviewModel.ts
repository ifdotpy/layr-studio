import type { Task, WorkBody } from "./TasksStore.ts";
import { errorText, type PostBody } from "../../../../api.ts";
import {
  DEFAULT_GATE_TIMEOUT_MS,
  MAX_GATE_TIMEOUT_MS,
  MAX_RESULT_SCHEMA_BYTES,
} from "../../../../generated/taskReview.ts";

export type TaskConfiguration = Extract<
  PostBody<"/api/work">,
  { action: "create" | "update" }
>;
export type Gate = TaskConfiguration["gate"];
export const prettyJson = (value: unknown) => JSON.stringify(value, null, 2);

export function gateFields(gate: Gate) {
  return {
    command: typeof gate === "string" ? gate : gate?.command || "",
    timeout: String(
      typeof gate === "object" && gate?.timeout_ms
        ? gate.timeout_ms
        : DEFAULT_GATE_TIMEOUT_MS,
    ),
  };
}

export function parseGate(
  command: string,
  timeout: string,
): Exclude<Gate, undefined> {
  if (!command.trim()) return null;
  const milliseconds = Number(timeout);
  if (
    !Number.isInteger(milliseconds) ||
    milliseconds < 1 ||
    milliseconds > MAX_GATE_TIMEOUT_MS
  )
    throw new Error(
      `The gate timeout must be an integer from 1 to ${MAX_GATE_TIMEOUT_MS} milliseconds.`,
    );
  return { command: command.trim(), timeout_ms: milliseconds };
}

export function parseResultSchema(
  text: string,
): TaskConfiguration["result_schema"] {
  if (!text.trim()) return undefined;
  if (new TextEncoder().encode(text).length > MAX_RESULT_SCHEMA_BYTES)
    throw new Error(
      `The result schema must not exceed ${MAX_RESULT_SCHEMA_BYTES} UTF-8 bytes.`,
    );
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error("The result schema must be valid JSON.");
  }
  if (
    typeof value !== "boolean" &&
    (!value || typeof value !== "object" || Array.isArray(value))
  )
    throw new Error("The result schema must be a JSON object or boolean.");
  return value as TaskConfiguration["result_schema"];
}

export function taskDraft(task?: Task) {
  return {
    title: task?.title || "",
    description: task?.description || "",
    owner: task?.owner || "",
    dependencies: [...(task?.dependencies || [])],
    status: (task ? "" : "ready") as "ready" | "blocked" | "",
    mode: task ? "custom" : "inherit",
    gateInput: gateFields(task?.gate),
    schema:
      task?.resultSchema === undefined ? "" : prettyJson(task.resultSchema),
  };
}
export function taskSchemaError(schema: string) {
  try {
    parseResultSchema(schema);
    return "";
  } catch (failure) {
    return errorText(failure);
  }
}

export function taskBody(
  draft: ReturnType<typeof taskDraft>,
  task: Task | undefined,
  values: FormData,
): WorkBody {
  const status = values.get("status");
  const resultSchema = parseResultSchema(draft.schema);
  if (task?.resultSchema !== undefined && resultSchema === undefined)
    throw new Error(
      "Use true to allow any JSON result. An existing result schema cannot be removed.",
    );
  const dependencies = values.getAll("dependencies").map(String);
  const fields = {
    title: draft.title.trim(),
    description: String(values.get("description") || ""),
    owner: String(values.get("owner") || "").trim() || null,
    dependencies:
      dependencies.length === draft.dependencies.length &&
      draft.dependencies.every((id) => dependencies.includes(id))
        ? draft.dependencies
        : dependencies,
    ...(status === "ready" || status === "blocked" ? { status } : {}),
    ...(draft.mode === "custom" && {
      gate: parseGate(draft.gateInput.command, draft.gateInput.timeout),
    }),
    ...(resultSchema === undefined ? {} : { result_schema: resultSchema }),
  } satisfies Omit<TaskConfiguration, "action">;
  return task
    ? {
        action: "update",
        task_id: task.id,
        version: task.version,
        ...fields,
      }
    : { action: "create", ...fields };
}
