import { useEffect, useMemo } from "react";
import { reaction } from "mobx";
import {
  useServerApi,
  useServerResources,
  useServerStore,
} from "../servers/ServerServicesContext.tsx";
import { agentId, transcriptTail } from "../generated/observableResources.ts";
import { SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS } from "../generated/sharedValues.ts";
import { trimTranscriptPageCache } from "../transcriptPageBounds.ts";
import { TranscriptHistory } from "./historySession.ts";
import { useRoomHistory } from "./roomHistory.ts";
/** Standalone legacy history checks; production selection owns its session directly. */
export function useTranscriptHistory(
  id: string | null,
  managed = true,
  workspace = "",
  syncWorkspaceId = "",
  enabled = true,
) {
  const store = useServerStore();
  const api = useServerApi();
  const resources = useServerResources();
  // This is the previous paging cache, scoped to the same authenticated owner.
  const pages = useMemo(
    () => new Map<string, TranscriptHistory>(),
    [store, api, resources, managed, workspace, syncWorkspaceId],
  );
  const scope = id || "";
  const session = useMemo(() => {
    let page = pages.get(scope);
    if (!page)
      page = new TranscriptHistory(
        id,
        id
          ? store.transcript(
              agentId(id),
              transcriptTail(SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
            )
          : undefined,
        api,
        resources,
        managed,
      );
    pages.delete(scope);
    pages.set(scope, page);
    trimTranscriptPageCache(pages, scope);
    return page;
  }, [pages, scope, id, store, api, resources, managed]);
  useEffect(() => {
    if (!enabled) return;
    const stop = session.start();
    const boundCache = reaction(
      () => session.size,
      () => trimTranscriptPageCache(pages, scope),
    );
    return () => {
      boundCache();
      stop();
    };
  }, [session, enabled, pages, scope]);
  return session;
}

/** Root selection only; each resource keeps its own small navigation owner. */
export function useConversationHistory(
  id: string | null,
  kind: "agent" | "room" | "legacy",
  managed = false,
  workspace = "",
  syncWorkspaceId = "",
  enabled = true,
) {
  const agent = useTranscriptHistory(
    kind === "agent" ? id : null,
    managed,
    workspace,
    syncWorkspaceId,
    enabled && kind === "agent",
  );
  const room = useRoomHistory(
    kind !== "agent" ? id : null,
    workspace,
    enabled && kind !== "agent",
    kind === "legacy",
  );
  return kind === "agent" ? agent : room;
}
