import { displayError } from "../../errorPresentation.ts";
import type { components } from "../../generated/api.ts";
import type { DeepReadonly } from "../../deepReadonly.ts";
import type { StudioErrorItem } from "../../components/StudioErrors.tsx";
export type NativeNotice =
  | components["schemas"]["NativeNoticeRecord"]
  | components["schemas"]["ProviderVersionWarningDto"];
export function isInfoNotice(
  notice: DeepReadonly<NativeNotice>,
): notice is DeepReadonly<components["schemas"]["ProviderVersionWarningDto"]> {
  return "level" in notice && notice.level === "info";
}
export function warningTitle(value: unknown, fallback: string) {
  const text = displayError(value);
  if (/\b(version|baseline)\b/i.test(text)) {
    const provider = /\bClaude\b/i.test(text)
      ? "Claude"
      : /\bCodex\b/i.test(text)
        ? "Codex"
        : "Provider";
    return `${provider} version needs attention`;
  }
  const first = text.split(/\n|[.!?](?:\s|$)/)[0]?.trim();
  if (!first) return fallback;
  const words = first.split(/\s+/);
  return words.length > 8 ? `${words.slice(0, 8).join(" ")}…` : first;
}
export const ERROR_LEVELS: StudioErrorItem["level"][] = [
  "critical",
  "error",
  "warning",
  "log",
];
