import type { components } from "../generated/api.ts";

/** Presentation belongs to the renderer; precedence belongs to the Rust reason. */
export type ChatIndicatorKind =
  | "loading"
  | "answer"
  | "error"
  | "working"
  | "unread"
  | "paused"
  | "none";
export type ChatIndicator = {
  readonly kind: ChatIndicatorKind;
  readonly label: string;
};
export type OperationalStatus = components["schemas"]["AgentOperationalStatus"];
type WaitReasons = components["schemas"]["AgentWaitReasons"];

function waitLabel(wait: WaitReasons): string {
  const count = (value: number, noun: string) =>
    value ? `${value} ${noun}${value === 1 ? "" : "s"}` : "";
  const parts = [
    count(wait.agents, "agent"),
    count(wait.commands, "command"),
    count(wait.monitors, "monitor"),
    wait.event ? `event ${wait.event}` : "",
    count(wait.inputs, "input"),
  ].filter(Boolean);
  return `Waiting for ${parts.reduce((text, part, index) => (index === 0 ? part : `${text}${index === parts.length - 1 ? " and " : ", "}${part}`), "")}`;
}

export function formatOperationalStatus(
  status: OperationalStatus | undefined,
): ChatIndicator {
  if (status === undefined) return { kind: "loading", label: "" };
  switch (status.reason) {
    case "needs_input":
      return {
        kind: "answer",
        label: status.wait ? waitLabel(status.wait) : "Needs your answer",
      };
    case "waiting":
      return { kind: "working", label: waitLabel(status.wait) };
    case "waiting_background_command":
      return { kind: "working", label: "Waiting for background command" };
    case "working":
      return { kind: "working", label: "Working" };
    case "turn_ended":
      return { kind: "none", label: "Turn ended. Send a message to continue." };
    case "disk_space":
      return { kind: "paused", label: "Waiting for disk space" };
    case "failed":
      return { kind: "error", label: "Failed" };
    case "interrupted":
      return { kind: "paused", label: "Interrupted" };
    case "stopped":
      return { kind: "paused", label: "Stopped" };
    case "question_deferred":
      return { kind: "paused", label: "Question deferred" };
    case "unread_result":
      return { kind: "unread", label: "Unread result" };
    case "read_result":
      return { kind: "none", label: "Read" };
    case "ready":
      return { kind: "none", label: "Ready" };
  }
  const exhaustive: never = status;
  return exhaustive;
}
