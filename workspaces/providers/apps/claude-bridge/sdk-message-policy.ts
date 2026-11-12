import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import { CLAUDE_SDK_VERSION, type Disposition } from "./sdk-dispositions.ts";
const handled = { kind: "Handled" } as const;
const ignored = (reason: string) => ({ kind: "Ignored" as const, reason });
export const ignoredSdkMessages = {
  auth_status: ignored(
    "Account identity is verified through accountInfo before use.",
  ),
  tool_use_summary: ignored(
    "Individual tool items already retain their final output.",
  ),
  prompt_suggestion: ignored("Studio does not present prompt suggestions."),
  conversation_reset: ignored(
    "Durable SessionStart and init evidence own session transitions.",
  ),
};

export const ignoredSdkSystemMessages = {
  control_request_progress: ignored(
    "Studio waits own approval and tool request progress.",
  ),
  hook_started: ignored("SessionStart callbacks own transition evidence."),
  hook_progress: ignored("Hook progress does not change Studio turn state."),
  hook_response: ignored(
    "Hook callback results are consumed by the native SDK.",
  ),
  plugin_install: ignored("Plugin installation remains native configuration."),
  thinking_tokens: ignored(
    "Streamed thinking deltas already update assistant items.",
  ),
  session_state_changed: ignored(
    "Native session transitions require durable hook or init proof.",
  ),
  worker_shutting_down: ignored("Studio owns worker lifecycle independently."),
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
};

export type LoopControl = void | "stop";
type Handler<M, T = never, P = never> =
  | ((message: M, turn: T, parent: P) => LoopControl | Promise<LoopControl>)
  | ReturnType<typeof ignored>;
export type MessageHandlers<T = never, P = never> = {
  [M in SDKMessage as M["type"]]: Handler<M, T, P>;
};
export type SystemHandlers<T = never, P = never> = {
  [M in SDKMessage & { type: "system" } as M["subtype"]]: Handler<M, T, P>;
};
export type ResultHandlers<T = never, P = never> = {
  [M in SDKMessage & { type: "result" } as M["subtype"]]: Handler<M, T, P>;
};

export type MessageDisposition =
  | Disposition
  | {
      kind: "Unknown";
      provider: "claude";
      version: string;
      type: string;
      subtype?: string | undefined;
    };

// Only the native discriminator is inspected; payloads remain SDK-owned.
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

// The same entries own the decision and the callback. No parser or skip condition
// can stand in for a missing registration.
export function sdkMessageDisposition(
  message: SDKMessage,
  handlers: Partial<MessageHandlers>,
  systems: Partial<SystemHandlers>,
  results: Partial<ResultHandlers>,
  version = CLAUDE_SDK_VERSION,
): MessageDisposition {
  const record: Partial<Record<string, Handler<never>>> =
    message.type === "system"
      ? systems
      : message.type === "result"
        ? results
        : handlers;
  const key =
    message.type === "system" || message.type === "result"
      ? message.subtype
      : message.type;
  if (!Object.hasOwn(record, key)) return unknownMessage(message, version);
  const entry = record[key];
  return typeof entry === "function"
    ? handled
    : (entry ?? unknownMessage(message, version));
}
export function dispatchMessage<M, K extends keyof M, T, Parent>(
  message: M[K],
  handlers: { [P in keyof M]: Handler<M[P], T, Parent> },
  key: K,
  turn: T,
  parentToolUseId: Parent,
) {
  const handler = handlers[key];
  if (typeof handler === "function")
    return handler(message, turn, parentToolUseId);
}
// Maximum retained unknown SDK diagnostic identities per bridge process.
export const MAX_UNKNOWN_SDK_DIAGNOSTICS = 128;
export function createSdkMessageDiagnostics(
  report: (message: string) => void,
  version: string = CLAUDE_SDK_VERSION,
) {
  const recorded = new Set<string>();
  return (
    message: SDKMessage,
    handlers: Partial<MessageHandlers> = {},
    systems: Partial<SystemHandlers> = {},
    results: Partial<ResultHandlers> = {},
  ): MessageDisposition => {
    const disposition = sdkMessageDisposition(
      message,
      handlers,
      systems,
      results,
      version,
    );
    if (disposition.kind === "Unknown") {
      const key = JSON.stringify([
        disposition.provider,
        disposition.version,
        disposition.type,
        disposition.subtype,
      ]);
      if (recorded.size < MAX_UNKNOWN_SDK_DIAGNOSTICS && !recorded.has(key)) {
        recorded.add(key);
        report("Claude SDK unknown message: " + key + "\n");
      }
    }
    return disposition;
  };
}
