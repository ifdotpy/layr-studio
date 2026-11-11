import type { GetResult, PostBody } from "../../api.ts";
import type { Json } from "../../types.ts";
export type RuleRecord = GetResult<"/api/rules">["rules"][number];
export type RuleAction = NonNullable<PostBody<"/api/rules">["action"]>;
export type RuleDraft = {
  id: string;
  isNew?: boolean;
  name: string;
  kind: NonNullable<RuleRecord["kind"]>;
  intervalSeconds: number | string;
  minimumWorkers: number | string;
  durationMinutes: number | string;
  stallTimeoutSeconds: number | string;
  at: string;
  path: string;
  event: NonNullable<PostBody<"/api/rules">["event"]>;
  command: string;
  livenessCommand: string;
  text: string;
};

/** Form defaults, measured in active workers and elapsed minutes. */
export const lowWorkerDefaults = { minimumWorkers: 8, durationMinutes: 30 };

const ruleKinds = ["interval", "once", "file", "event", "low_workers"] as const;
const ruleEvents = [
  "worker_completed",
  "monitor_exit",
  "complaint",
  "work_review",
] as const;

export function isRuleKind(value: string): value is RuleDraft["kind"] {
  return ruleKinds.some((kind) => kind === value);
}

export function isRuleEvent(value: string): value is RuleDraft["event"] {
  return ruleEvents.some((event) => event === value);
}

type RuleIdentity = Pick<RuleDraft, "id" | "name" | "kind" | "isNew">;
function normalizeRuleDraft(value: Json, identity: RuleIdentity): RuleDraft {
  const string = (key: string) =>
    typeof value[key] === "string" ? value[key] : "";
  const number = (key: string, fallback: number) =>
    typeof value[key] === "number" || typeof value[key] === "string"
      ? value[key]
      : fallback;
  return {
    ...identity,
    intervalSeconds: number("intervalSeconds", 60),
    minimumWorkers: number("minimumWorkers", lowWorkerDefaults.minimumWorkers),
    durationMinutes: number(
      "durationMinutes",
      lowWorkerDefaults.durationMinutes,
    ),
    stallTimeoutSeconds: number("stallTimeoutSeconds", 1800),
    at: string("at"),
    path: string("path"),
    event:
      typeof value.event === "string" && isRuleEvent(value.event)
        ? value.event
        : "worker_completed",
    command: string("command"),
    livenessCommand: string("livenessCommand"),
    text: string("text"),
  };
}
export function parseRuleDraft(value: Json | null): RuleDraft | null {
  if (
    !value ||
    typeof value.id !== "string" ||
    typeof value.name !== "string" ||
    typeof value.kind !== "string" ||
    !isRuleKind(value.kind)
  )
    return null;
  return normalizeRuleDraft(value, {
    id: value.id,
    name: value.name,
    kind: value.kind,
    isNew: value.isNew === true,
  });
}
export function newRuleDraft(id: string): RuleDraft {
  return normalizeRuleDraft(
    { intervalSeconds: 300 },
    { id, name: "", kind: "interval", isNew: true },
  );
}
export function ruleDraftFromRecord(rule: RuleRecord): RuleDraft {
  const at = rule.at
    ? new Date(typeof rule.at === "number" ? rule.at * 1000 : rule.at)
        .toLocaleString("sv-SE")
        .slice(0, 16)
        .replace(" ", "T")
    : "";
  return normalizeRuleDraft(
    { ...rule, at },
    { id: rule.id, name: rule.name || "", kind: rule.kind || "interval" },
  );
}

export function ruleBodyFromDraft(
  draft: RuleDraft,
  agent: string,
): PostBody<"/api/rules"> {
  return {
    id: draft.id,
    agent,
    action: "save",
    name: draft.name,
    kind: draft.kind,
    intervalSeconds:
      draft.kind === "interval" ? Number(draft.intervalSeconds) : undefined,
    minimumWorkers:
      draft.kind === "low_workers" ? Number(draft.minimumWorkers) : undefined,
    durationMinutes:
      draft.kind === "low_workers" ? Number(draft.durationMinutes) : undefined,
    stallTimeoutSeconds: Number(draft.stallTimeoutSeconds),
    at: draft.kind === "once" ? new Date(draft.at).getTime() / 1000 : undefined,
    path: draft.kind === "file" ? draft.path : undefined,
    event: draft.kind === "event" ? draft.event : undefined,
    command: draft.kind === "low_workers" ? "" : draft.command,
    livenessCommand: draft.kind === "file" ? draft.livenessCommand : undefined,
    text: draft.text,
  };
}
