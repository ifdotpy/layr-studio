import { indexedDb } from "../indexedDb.ts";
const invalidations = new Map<
  string,
  Set<{ credentialId: string; accept: () => void }>
>();

/** Storage and credential notifications only; the sync client owns the cache scope. */
export function subscriptionCacheStorage(serverId: string, credentialId = "") {
  const storage = indexedDb(
    `studio-subscription-cache:${serverId}`,
    "subscriptions",
    1,
  );
  for (const other of invalidations.get(serverId) ?? [])
    if (other.credentialId !== credentialId) other.accept();
  return {
    ...storage,
    serverId,
    credentialId,
    clearScope(owner: string, valid: () => boolean) {
      const prefix = `scope:${encodeURIComponent(owner)}:`;
      return storage.scan((values) => ({
        value: undefined,
        remove: valid()
          ? values
              .filter(
                ([key]) =>
                  typeof key === "string" &&
                  (!key.startsWith("scope:") || key.startsWith(prefix)),
              )
              .map(([key]) => key)
          : [],
      }));
    },
    onInvalidation(accept: () => void) {
      let listeners = invalidations.get(serverId);
      if (!listeners) invalidations.set(serverId, (listeners = new Set()));
      const listener = { credentialId, accept };
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
        if (!listeners.size) invalidations.delete(serverId);
      };
    },
  };
}
export async function invalidateSubscriptionCache(serverId: string) {
  for (const listener of invalidations.get(serverId) ?? []) listener.accept();
  await indexedDb(
    `studio-subscription-cache:${serverId}`,
    "subscriptions",
    1,
  ).scan((values) => ({
    value: undefined,
    remove: values.map(([key]) => key),
  }));
}
