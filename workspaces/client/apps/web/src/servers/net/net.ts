import { createMessageQueue, type QueueOptions } from "./queue.ts";
import type { createLocalDraftStorage } from "../../sync/localDraft.ts";
import type { GetOptions, ServerApi } from "../../api.ts";
import type { createServerStore } from "../../store/server.ts";
import { NetStatus } from "./status.ts";

type Store = ReturnType<typeof createServerStore>;
/** One server's existing keyed reads and queue owner, with its API bound once. */
export function createNet(
  api: ServerApi,
  store: () => Store,
  status = new NetStatus(),
  queueStorage?: {
    storage: Storage;
    updateQueueRequest: ReturnType<
      typeof createLocalDraftStorage
    >["updateQueueRequest"];
  },
) {
  return {
    reads: {
      transcriptSearch: (
        query: Parameters<Store["reads"]["transcriptSearch"]>[1],
      ) => store().reads.transcriptSearch(api, query),
      file: (query: Parameters<Store["reads"]["file"]>[1]) =>
        store().reads.file(api, query),
      analytics: (query: Parameters<Store["reads"]["analytics"]>[1]) =>
        store().reads.analytics(api, query),
      search: (query: Parameters<Store["reads"]["search"]>[1]) =>
        store().reads.search(api, query),
      directories: (query: Parameters<Store["reads"]["directories"]>[1]) =>
        store().reads.directories(api, query),
      projectLocations: (
        query: Parameters<Store["reads"]["projectLocations"]>[1],
      ) => store().reads.projectLocations(api, query),
      skills: (
        query: Parameters<Store["reads"]["skills"]>[1],
        context?: Parameters<Store["reads"]["skills"]>[2],
      ) => store().reads.skills(api, query, context),
      transcriptPage: (
        query: Parameters<Store["reads"]["transcriptPage"]>[1],
      ) => store().reads.transcriptPage(api, query),
    },
    queue: (options: QueueOptions, locks: Set<string>) => {
      if (!queueStorage) throw new Error("Queue storage must be provided.");
      return createMessageQueue(
        options,
        store(),
        api,
        queueStorage.storage,
        queueStorage.updateQueueRequest,
        locks,
      );
    },
    exportAnalytics: (query: GetOptions<"/api/analytics">["query"]) =>
      api.apiDownload("/api/analytics", query),
    status,
  };
}
export type Net = ReturnType<typeof createNet>;
