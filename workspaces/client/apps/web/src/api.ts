/** Public data, presentation and local-storage contracts. Transport stays in servers/net. */
import { NetworkTimeoutError } from "./apiErrors.ts";
import { createApiStorage } from "./apiStorage.ts";
import { serverLocalStorage } from "./servers/storage.ts";
export * from "./apiErrors.ts";
export type * from "./apiTypes.ts";
export type {
  ApiOptions,
  ApiReadMetadata,
  GetOptions,
  PostOptions,
  QueueView,
  QueueItemDto,
  ServerApi,
  ApiScopeOptions,
  get,
  post,
  syncGet,
  syncPost,
  createApiScope,
} from "./servers/net/api.ts";
export const READ_DEADLINE_MS = 120_000;
export const { saved, save, stableRequestId, clearStableRequestId } =
  createApiStorage(serverLocalStorage);
/** Send one exact write until the server answers it. A timeout means a slow server, not
 * a failure: each attempt has the same request identity, so the server applies it once. */
export async function untilAnswered<T>(send: () => Promise<T>): Promise<T> {
  for (;;) {
    try {
      return await send();
    } catch (error) {
      if (!(error instanceof NetworkTimeoutError)) throw error;
    }
  }
}
