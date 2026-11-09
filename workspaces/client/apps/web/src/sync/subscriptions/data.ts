import type { components } from "../../generated/api.ts";
import { watchResourceReads } from "./reads.ts";
import type {
  ResourceData,
  ResourceRef,
  ResourceProgress,
} from "./contracts.ts";
import {
  selectResourceData,
  type ResourceDataSelectors,
} from "./selectData.ts";
import type { ObserveSubscriptionData } from "./module.ts";
import type { ResourceVersion } from "../resourceEvents.ts";
import type {
  createSyncClient,
  TranscriptProjectionPayload,
} from "../client.ts";
import type { SubscriptionTransport } from "./store.ts";

const viewSelectors: Required<
  Pick<
    ResourceDataSelectors<ResourceVersion | undefined>,
    "processes" | "complaint" | "room" | "terminal-output"
  >
> = {
  processes: (_ref, version) => version?.viewSnapshot?.processes,
  complaint: (_ref, version) => version?.viewSnapshot?.complaint,
  room: (_ref, version) =>
    version?.viewSnapshot?.room ?? version?.viewSnapshot?.legacyMessages,
  "terminal-output": (_ref, version) => version?.viewSnapshot?.terminalOutput,
};
const transcriptSelectors: ResourceDataSelectors<TranscriptProjectionPayload> =
  {
    transcript: (_ref, data) => data ?? undefined,
  };

function hasProgressKind<K extends ResourceProgress["resource"]["kind"]>(
  progress: ResourceProgress | undefined,
  kind: K,
): progress is Extract<ResourceProgress, { resource: { kind: K } }> {
  return progress?.resource.kind === kind;
}
function progressFor<K extends ResourceProgress["resource"]["kind"]>(
  kind: K,
  version?: ResourceVersion,
): Extract<ResourceProgress, { resource: { kind: K } }> | undefined {
  const progress = version?.progress;
  return hasProgressKind(progress, kind) ? progress : undefined;
}
const transientSelectors: ResourceDataSelectors<ResourceVersion | undefined> = {
  "auth-progress": (_ref, version) =>
    progressFor("auth-progress", version)?.value,
  "rename-progress": (_ref, version) =>
    progressFor("rename-progress", version)?.value,
  "multi-server": (_ref, version) => version?.discoverySnapshot,
  "native-layr-settings": (_ref, version) => version?.nativeLayrSettings,
  voice: (_ref, version) => version?.voiceRecords,
  drafts: (_ref, version) => version?.draftChanges,
};

/** Apply completed stream values; voice and auth actions retain their explicit reads. */
export function createDataObserver(
  sync: () => Pick<
    ReturnType<typeof createSyncClient>,
    "subscribeTranscriptProjection"
  >,
  events: SubscriptionTransport,
  observeEntities?: ObserveSubscriptionData,
  api?: Pick<
    ReturnType<typeof import("../../api.ts").createApiScope>,
    "get" | "post"
  >,
): ObserveSubscriptionData {
  return <R extends ResourceRef>(
    ref: R,
    accept: (
      data: ResourceData<R> | undefined,
      appliedVersion?: ResourceVersion,
    ) => void,
    failed: (error: unknown) => void,
  ) => {
    if (ref.kind === "state")
      return observeEntities?.(ref, accept, failed) ?? (() => {});
    if (ref.kind === "transcript") {
      return sync().subscribeTranscriptProjection(
        `transcript:${ref.agentId}`,
        (data) => {
          accept(selectResourceData(transcriptSelectors, ref, data));
        },
        (error) => {
          if (error !== null) failed(error);
        },
        true,
        ref,
      );
    }
    if (api && (ref.kind === "voice" || ref.kind === "auth-progress")) {
      let stopped = false;
      let epoch: string | undefined;
      let voice: components["schemas"]["VoiceRecordBatch"] | undefined;
      const stop = watchResourceReads(
        ref,
        async (version) => {
          if (!version) return;
          if (ref.kind === "auth-progress") {
            const progress = progressFor("auth-progress", version);
            if (!progress) return;
            const value =
              progress.value ??
              (await api.get("/api/accounts/claude/login", {
                query: { request_id: ref.requestId },
              }));
            if (!stopped && value.requestId === ref.requestId)
              accept(
                selectResourceData(
                  { "auth-progress": () => value },
                  ref,
                  undefined,
                ),
                version,
              );
            return;
          }
          const reset = epoch !== version.epoch;
          const after = reset ? 0 : (voice?.cursor ?? 0);
          const inline = version.voiceRecords;
          const batch =
            inline && inline.after <= after
              ? inline
              : {
                  ...(await api.post(
                    "/api/voice/records",
                    { agent: ref.agentId, after },
                    { timeoutMs: 15000 },
                  )),
                  after,
                };
          if (stopped) return;
          voice = {
            ...batch,
            cursor: Math.max(after, batch.cursor),
            records: [
              ...new Map(
                [
                  ...(reset ? [] : (voice?.records ?? [])),
                  ...batch.records,
                ].map((row) => [row.id, row]),
              ).values(),
            ],
          };
          epoch = version.epoch;
          accept(
            selectResourceData({ voice: () => voice }, ref, undefined),
            version,
          );
        },
        failed,
        { events, waitForBaseline: true },
      );
      return () => {
        stopped = true;
        stop();
      };
    }
    const viewValue = (version?: ResourceVersion) =>
      selectResourceData(viewSelectors, ref, version);
    const transient = (version?: ResourceVersion) => {
      const viewData = viewValue(version);
      if (viewData !== undefined) {
        accept(viewData, version);
        return;
      }
      if (version && ref.kind in viewSelectors) {
        failed(
          new Error(
            "Live updates for this view are unavailable. Reconnect to retry.",
          ),
        );
        return;
      }
      const data = selectResourceData(transientSelectors, ref, version);
      if (data !== undefined) accept(data, version);
    };
    const stop = events.watchResourceChanges(ref, transient);
    const stopEntities = observeEntities?.(ref, accept, failed);
    return () => {
      stop();
      stopEntities?.();
    };
  };
}
