import { outboxRequests } from "./browserPersistence.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { API_SCHEMA_HASH } from "./generated/apiSchema.ts";
export function createApiStorage(localStorage: Storage) {
  const cacheKey = (key: string) =>
    [
      "codex-confirmed-chats",
      "codex-sidebar-collapsed",
      "codex-team-open",
      "codex-mobile-opened",
    ].includes(key) ||
    [
      "codex.terminal.",
      "codex-desktop-opened:",
      "project-last-server:",
      "codex-chat-seen:",
    ].some((prefix) => key.startsWith(prefix)) ||
    key.endsWith(":tools-v3");
  function saved<T>(key: string, fallback: T): T {
    try {
      const value = JSON.parse(localStorage.getItem(key) || "null");
      if (cacheKey(key))
        return value?.schemaHash === API_SCHEMA_HASH
          ? (value.value ?? fallback)
          : fallback;
      return value ?? fallback;
    } catch {
      return fallback;
    }
  }

  function save(key: string, value: unknown) {
    try {
      const raw = JSON.stringify(
        cacheKey(key) ? { schemaHash: API_SCHEMA_HASH, value } : value,
      );
      // Keep the storage adapter's pending journal intact when the UI repeats a value.
      if (localStorage.getItem(key) !== raw) localStorage.setItem(key, raw);
    } catch {
      /* Server data remains available if browser storage is full. */
    }
  }

  function stableRequestId(key: string): string {
    const outbox = outboxRequests(localStorage);
    const raw = outbox.get("stableRequest", key);
    const savedId: unknown = raw === null ? null : JSON.parse(raw);
    if (savedId !== null) {
      if (typeof savedId !== "string" || !savedId)
        throw new Error(
          "The saved request identity is invalid. Resolve it before retrying.",
        );
      return savedId;
    }
    const requestId = randomUUID();
    outbox.set("stableRequest", key, JSON.stringify(requestId));
    return requestId;
  }

  function clearStableRequestId(key: string) {
    outboxRequests(localStorage).remove("stableRequest", key);
  }

  return { saved, save, stableRequestId, clearStableRequestId };
}
