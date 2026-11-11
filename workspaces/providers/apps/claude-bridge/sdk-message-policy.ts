import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import { CLAUDE_SDK_VERSION, type Disposition } from "./sdk-dispositions.ts";

const handled = { kind: "Handled" } as const;
const ignored = (reason: string): Disposition => ({ kind: "Ignored", reason });

export const handledSdkMessages = {
  assistant: handled,
  user: handled,
  result: handled,
  system: handled,
  stream_event: handled,
  tool_progress: handled,
  auth_status: ignored(
    "Account identity is verified through accountInfo before use.",
  ),
  tool_use_summary: ignored(
    "Individual tool items already retain their final output.",
  ),
  rate_limit_event: handled,
  prompt_suggestion: ignored("Studio does not present prompt suggestions."),
  conversation_reset: ignored(
    "Durable SessionStart and init evidence own session transitions.",
  ),
} satisfies Record<SDKMessage["type"], Disposition>;

export const handledSdkSystemMessages = {
  init: handled,
  compact_boundary: handled,
  status: handled,
  api_retry: handled,
  control_request_progress: ignored(
    "Studio waits own approval and tool request progress.",
  ),
  model_refusal_fallback: handled,
  model_refusal_no_fallback: handled,
  local_command_output: handled,
  hook_started: ignored("SessionStart callbacks own transition evidence."),
  hook_progress: ignored("Hook progress does not change Studio turn state."),
  hook_response: ignored(
    "Hook callback results are consumed by the native SDK.",
  ),
  plugin_install: ignored("Plugin installation remains native configuration."),
  task_notification: handled,
  task_started: handled,
  task_updated: handled,
  task_progress: handled,
  background_tasks_changed: handled,
  thinking_tokens: ignored(
    "Streamed thinking deltas already update assistant items.",
  ),
  session_state_changed: ignored(
    "Native session transitions require durable hook or init proof.",
  ),
  worker_shutting_down: ignored("Studio owns worker lifecycle independently."),
  commands_changed: ignored("Commands are read from initialization on demand."),
  notification: handled,
  files_persisted: ignored(
    "Studio verifies checkpoint files through native history.",
  ),
  memory_recall: ignored(
    "Native memory content remains part of the SDK prompt.",
  ),
  elicitation_complete: ignored(
    "The awaited elicitation callback owns completion.",
  ),
  permission_denied: ignored(
    "The awaited tool permission callback owns denial.",
  ),
  mirror_error: handled,
  informational: handled,
} satisfies Record<
  Extract<SDKMessage, { type: "system" }>["subtype"],
  Disposition
>;

export type MessageDisposition =
  | Disposition
  | {
      kind: "Unknown";
      provider: "claude";
      version: string;
      type: string;
      subtype?: string | undefined;
    };

// The default is reachable only when runtime SDK data is newer than its pinned
// declarations. The never proof still rejects every newly declared variant.
function unknownMessage(value: unknown, version: string): MessageDisposition {
  const row = value !== null && typeof value === "object" ? value : {};
  return {
    kind: "Unknown",
    provider: "claude",
    version,
    type:
      "type" in row && typeof row.type === "string" ? row.type : "<invalid>",
    subtype:
      "subtype" in row && typeof row.subtype === "string"
        ? row.subtype
        : undefined,
  };
}
export function sdkMessageDisposition(
  message: SDKMessage,
  version: string = CLAUDE_SDK_VERSION,
): MessageDisposition {
  switch (message.type) {
    case "assistant":
      return handledSdkMessages[message.type];
    case "user":
      return handledSdkMessages[message.type];
    case "stream_event":
      return handledSdkMessages[message.type];
    case "tool_progress":
      return handledSdkMessages[message.type];
    case "auth_status":
      return handledSdkMessages[message.type];
    case "tool_use_summary":
      return handledSdkMessages[message.type];
    case "rate_limit_event":
      return handledSdkMessages[message.type];
    case "prompt_suggestion":
      return handledSdkMessages[message.type];
    case "conversation_reset":
      return handledSdkMessages[message.type];
    case "result":
      switch (message.subtype) {
        case "success":
          return handled;
        case "error_during_execution":
          return handled;
        case "error_max_turns":
          return handled;
        case "error_max_budget_usd":
          return handled;
        case "error_max_structured_output_retries":
          return handled;
        default:
          message satisfies never;
          return unknownMessage(message, version);
      }
    case "system":
      switch (message.subtype) {
        case "init":
          return handledSdkSystemMessages[message.subtype];
        case "compact_boundary":
          return handledSdkSystemMessages[message.subtype];
        case "status":
          return handledSdkSystemMessages[message.subtype];
        case "api_retry":
          return handledSdkSystemMessages[message.subtype];
        case "control_request_progress":
          return handledSdkSystemMessages[message.subtype];
        case "model_refusal_fallback":
          return handledSdkSystemMessages[message.subtype];
        case "model_refusal_no_fallback":
          return handledSdkSystemMessages[message.subtype];
        case "local_command_output":
          return handledSdkSystemMessages[message.subtype];
        case "hook_started":
          return handledSdkSystemMessages[message.subtype];
        case "hook_progress":
          return handledSdkSystemMessages[message.subtype];
        case "hook_response":
          return handledSdkSystemMessages[message.subtype];
        case "plugin_install":
          return handledSdkSystemMessages[message.subtype];
        case "task_notification":
          return handledSdkSystemMessages[message.subtype];
        case "task_started":
          return handledSdkSystemMessages[message.subtype];
        case "task_updated":
          return handledSdkSystemMessages[message.subtype];
        case "task_progress":
          return handledSdkSystemMessages[message.subtype];
        case "background_tasks_changed":
          return handledSdkSystemMessages[message.subtype];
        case "thinking_tokens":
          return handledSdkSystemMessages[message.subtype];
        case "session_state_changed":
          return handledSdkSystemMessages[message.subtype];
        case "worker_shutting_down":
          return handledSdkSystemMessages[message.subtype];
        case "commands_changed":
          return handledSdkSystemMessages[message.subtype];
        case "notification":
          return handledSdkSystemMessages[message.subtype];
        case "files_persisted":
          return handledSdkSystemMessages[message.subtype];
        case "memory_recall":
          return handledSdkSystemMessages[message.subtype];
        case "elicitation_complete":
          return handledSdkSystemMessages[message.subtype];
        case "permission_denied":
          return handledSdkSystemMessages[message.subtype];
        case "mirror_error":
          return handledSdkSystemMessages[message.subtype];
        case "informational":
          return handledSdkSystemMessages[message.subtype];
        default:
          message satisfies never;
          return unknownMessage(message, version);
      }
    default:
      message satisfies never;
      return unknownMessage(message, version);
  }
}

export function createSdkMessageDiagnostics(
  report: (message: string) => void,
  version: string = CLAUDE_SDK_VERSION,
) {
  const recorded = new Set<string>();
  return (message: SDKMessage): MessageDisposition => {
    const disposition = sdkMessageDisposition(message, version);
    if (disposition.kind === "Unknown") {
      const key = JSON.stringify([
        disposition.provider,
        disposition.version,
        disposition.type,
        disposition.subtype,
      ]);
      if (!recorded.has(key)) {
        recorded.add(key);
        report("Claude SDK unknown message: " + key + "\n");
      }
    }
    return disposition;
  };
}
