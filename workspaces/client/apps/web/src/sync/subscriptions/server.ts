import { SignedEventSource } from "../../servers/net/eventSource.ts";
import { routesFor } from "../../servers/net/routes.ts";
import { hasNativeClient } from "../../native/client.ts";
import { NativeEventSource } from "../../native/stream.ts";
import type { createApiScope } from "../../api.ts";
import type { createSyncClient } from "../client.ts";
import {
  createResourceEvents,
  type ResourceEventsOptions,
} from "../resourceEvents.ts";
import { MemoryTransferStorage } from "../entityTransfer.ts";
import { createSubscriptions } from "./module.ts";
import { createDataObserver } from "./data.ts";
import { createEntityDataObserver } from "./entityData.ts";

type ServerSubscriptionOptions = {
  api: ResourceEventsOptions["api"] &
    Pick<ReturnType<typeof createApiScope>, "fetchApiRequest" | "post">;
  tokenRates: ResourceEventsOptions["tokenRates"];
  sync: () => ReturnType<typeof createSyncClient>;
  owner: { id: string; origin: string };
  /** The native core owns this server's stream and projections: no browser storage, locks or channels. */
  nativeCore?: () => boolean;
};

/** The server service supplies ownership and cache access, never stream details. */
export function createServerSubscriptions(options: ServerSubscriptionOptions) {
  const { api, tokenRates, sync, owner, nativeCore } = options;
  const transport = createResourceEvents({
    api,
    tokenRates,
    onCachePolicies: (decisions) => sync().setCachePolicies(decisions),
    onCachePoliciesReset: () => sync().clearCachePolicies(),
    sync: {
      syncDatabase: () => sync().syncDatabase(),
      persistResourceEntityChanges: (...args) =>
        sync().persistResourceEntityChanges(...args),
      readSubscriptionCursor: (ref) => sync().readSubscriptionCursor(ref),
      persistStagedEntityTransfer: (...args) =>
        sync().persistStagedEntityTransfer(...args),
    },
    // The local server's requests use the native client there (serverServices.ts).
    transferStorage: () =>
      nativeCore?.() ? new MemoryTransferStorage() : undefined,
    // Each connection of a remote server reports the routes it answers on.
    ...(owner.id === "local"
      ? {}
      : {
          onRoutes: (routes: readonly string[]) =>
            routesFor(owner).learn(routes),
        }),
    openEvents: (url) =>
      owner.id === "local" && hasNativeClient()
        ? new NativeEventSource(url)
        : new SignedEventSource(
            new URL(url, owner.origin).href,
            api.fetchApiRequest,
          ),
  });
  // A move to a faster route replaces the stream.
  const stopRoutes =
    owner.id === "local"
      ? undefined
      : routesFor(owner).onChange(() => transport.replaceStream());
  const subscriptions = createSubscriptions(
    {
      ...transport,
      dispose() {
        stopRoutes?.();
        transport.dispose();
      },
    },
    createDataObserver(
      sync,
      transport,
      createEntityDataObserver(
        () => sync().syncDatabase(),
        (report) => sync().retainStateDelivery(report),
      ),
      api,
    ),
  );
  return subscriptions;
}
