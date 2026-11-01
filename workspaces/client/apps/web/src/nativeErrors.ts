import { claudeFailureView } from "./claudeFailure.ts";
import { errorDetails } from "./errorPresentation.ts";
import {
  DISK_FULL_ERROR_PATTERN,
  LEGACY_VM_PROVIDER_DISCONNECTED_TURN_ERROR,
  VM_PROVIDER_DISCONNECTED_TURN_ERROR,
} from "./generated/sharedValues.ts";
import type { Agent } from "./types";
import type { KnownCodexErrorKind } from "studio-claude-bridge/provider-errors.generated.ts";
import {
  knownCodexErrorKind,
  rawCodexErrorKind,
} from "studio-claude-bridge/provider-errors.ts";

// A Studio condition, not a Codex error kind: the state volume has no free space.
const DISK_FULL_PATTERN = new RegExp(DISK_FULL_ERROR_PATTERN, "i");
export const DISK_FULL_TITLE = "Disk is full";
const DISK_FULL_MESSAGE =
  "Studio holds this agent and resumes it when free space returns.";
const DISK_FULL_HINT =
  "Free disk space on the server. Queued work starts again on its own.";

// Every known vendor tag needs a hint; future raw records retain their details.
export const nativeErrorHints = {
  contextWindowExceeded: "Compact the context or start a new chat.",
  sessionBudgetExceeded: "Check the session budget before continuing.",
  usageLimitExceeded: "Check this account's limits and reset time.",
  rateLimitExceeded:
    "Check this account's limits and available recovery options.",
  flexUnavailable:
    "Try again later or choose another model. The flexible processing tier is unavailable.",
  tooManyDenials:
    "Review the denied actions and approval settings before continuing.",
  serverOverloaded: "Try later or select another model.",
  cyberPolicy:
    "Start a new chat with a narrower or rephrased task, or use another provider or model.",
  misalignmentPolicyViolation:
    "Codex could not confirm that the agent followed your instructions safely. Start a new chat or open another chat.",
  httpConnectionFailed: "Check the connection before continuing.",
  responseStreamConnectionFailed: "Check the connection before continuing.",
  internalServerError: "Review the last result before continuing.",
  unauthorized: "Sign in again to this chat's account.",
  badRequest: "Check the request and the selected model settings.",
  threadRollbackFailed:
    "Check the conversation before trying another rollback.",
  sandboxError: "Check the command permissions and the project folder.",
  responseStreamDisconnected: "Review the last result before continuing.",
  responseTooManyFailedAttempts:
    "Codex stopped retrying. Review the last result before continuing.",
  activeTurnNotSteerable: "Queue the message until the current turn ends.",
  other: "Review the error details before continuing.",
} satisfies Record<KnownCodexErrorKind, string>;

export function nativeErrorKind(value: unknown): string {
  return rawCodexErrorKind(objectRecord(value)?.codexErrorInfo) || "";
}

export function isCyberPolicyRefusal(value: unknown): boolean {
  return knownNativeErrorKind(value) === "cyberPolicy";
}

export function knownNativeErrorKind(
  value: unknown,
): KnownCodexErrorKind | undefined {
  return knownCodexErrorKind(nativeErrorKind(value));
}

function objectRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

type ReadableError = { message: string; biological: boolean };

function readableError(value: unknown): ReadableError | undefined {
  if (typeof value === "string")
    return value.trim() ? { message: value, biological: false } : undefined;
  const record = objectRecord(value);
  if (!record) return undefined;
  const biological = record.code === "bio_policy";
  if (typeof record.message === "string" && record.message.trim())
    return { message: record.message, biological };
  return biological
    ? { message: "Codex reported an error.", biological }
    : undefined;
}

export function nativeThreadError(
  agent?: Pick<Agent, "threadId" | "nativeThreadBlock" | "error">,
): unknown {
  const block = objectRecord(agent?.nativeThreadBlock);
  if (agent?.threadId && block?.threadId === agent.threadId && "error" in block)
    return block.error;
  return knownNativeErrorKind(agent?.error) === "misalignmentPolicyViolation"
    ? agent?.error
    : undefined;
}

// True when an error means the disk is full (ENOSPC, SQLite SQLITE_FULL).
export function isDiskFullError(value: unknown): boolean {
  const data = value;
  const record = objectRecord(data);
  if (record?.codexErrorInfo === "diskFull") return true;
  const readable = readableError(data);
  return DISK_FULL_PATTERN.test(
    readable?.message ?? (typeof value === "string" ? value : ""),
  );
}

export function nativeErrorView(value: unknown, planType?: string) {
  const view = codexErrorView(value, planType);
  if (view.message === LEGACY_VM_PROVIDER_DISCONNECTED_TURN_ERROR)
    return {
      ...view,
      message: VM_PROVIDER_DISCONNECTED_TURN_ERROR,
      details: view.details || view.message,
    };
  if (!isDiskFullError(value)) {
    const record = objectRecord(value);
    const claude = claudeFailureView(record?.claudeFailure);
    if (!claude) return view;
    return {
      ...view,
      ...claude,
      message: ["usageLimitExceeded", "rateLimitExceeded"].includes(view.kind)
        ? view.message
        : claude.message,
    };
  }
  return {
    ...view,
    message: DISK_FULL_MESSAGE,
    title: DISK_FULL_TITLE,
    hint: DISK_FULL_HINT,
    severity: "warning",
  };
}

function codexErrorView(value: unknown, planType?: string) {
  const data = value;
  const record = objectRecord(data);
  const structured = record !== null;
  const rawKind = nativeErrorKind(data);
  const kind = knownCodexErrorKind(rawKind);
  const readable = readableError(data);
  const rawMessage =
    readable?.message ??
    (typeof value === "string" && structured && !rawKind ? value : undefined);
  const message =
    kind === "cyberPolicy"
      ? "Codex refused this turn under its cybersecurity policy."
      : kind === "tooManyDenials"
        ? "Codex stopped this turn after too many denied actions."
        : kind === "flexUnavailable"
          ? "The selected model's flexible processing tier is unavailable right now."
          : typeof rawMessage === "string" && rawMessage.trim()
            ? rawMessage
            : kind === "serverOverloaded"
              ? "Codex is currently experiencing high load."
              : structured
                ? "Codex reported an error."
                : "";
  const policy = kind === "cyberPolicy" || Boolean(readable?.biological);
  const links: { label: string; href: string }[] = [];
  if (policy) {
    const individual = ["free", "go", "plus", "pro", "pro_lite"].includes(
      planType || "",
    );
    links.push(
      {
        label: "Trusted Access",
        href:
          kind === "cyberPolicy"
            ? individual
              ? "https://chatgpt.com/cyber/"
              : "https://openai.com/form/enterprise-trusted-access-for-cyber/"
            : "https://chatgpt.com/r/b749fb02595e04c3007a54375f3f4374",
      },
      {
        label: "Learn more",
        href: "https://help.openai.com/en/articles/20001326",
      },
    );
  }
  return {
    kind: rawKind,
    message,
    title:
      kind === "cyberPolicy"
        ? "Cybersecurity policy refusal"
        : kind === "misalignmentPolicyViolation"
          ? "Chat stopped as a precaution"
          : kind === "tooManyDenials"
            ? "Too many actions denied"
            : kind === "flexUnavailable"
              ? "Flexible processing unavailable"
              : kind === "usageLimitExceeded"
                ? "Usage limit reached"
                : kind === "rateLimitExceeded"
                  ? "Rate limit reached"
                  : policy
                    ? "This content can't be shown"
                    : "",
    hint:
      policy && kind !== "cyberPolicy"
        ? "Codex restricts biological research requests that could pose safety risks. Eligible researchers can apply for Trusted Access."
        : kind
          ? nativeErrorHints[kind]
          : rawKind
            ? nativeErrorHints.other
            : "",
    severity:
      kind === "serverOverloaded" || kind === "activeTurnNotSteerable"
        ? "warning"
        : policy
          ? "info"
          : "error",
    details:
      typeof value === "string" && value !== message
        ? value
        : structured
          ? errorDetails(data)
          : "",
    additionalDetails:
      record && typeof record.additionalDetails === "string"
        ? record.additionalDetails
        : "",
    links,
  };
}
