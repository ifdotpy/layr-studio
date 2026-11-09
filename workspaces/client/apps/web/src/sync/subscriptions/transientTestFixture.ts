import { vi } from "vitest";
import { createSubscriptions } from "./module.ts";
import { createDataObserver } from "./data.ts";
import { createServerStore } from "../../store/server.ts";
import { subscriptionKey, type ResourceRef } from "./contracts.ts";
import type { ResourceVersion } from "./events.ts";

export function transientTestFixture(recovery?: {
  authProgress(
    requestId: string,
  ): Promise<
    NonNullable<
      import("./contracts.ts").ResourceData<
        Extract<ResourceRef, { kind: "auth-progress" }>
      >
    >
  >;
  voice(
    agentId: string,
    after: number,
  ): Promise<
    import("../../generated/api.ts").components["schemas"]["VoiceRecordBatch"]
  >;
}) {
  const listeners = new Map<string, Set<(version?: ResourceVersion) => void>>();
  const connections = new Set<
    Parameters<
      import("./store.ts").SubscriptionTransport["watchResourceConnection"]
    >[0]
  >();
  const transport = {
    watchResourceChanges: vi.fn(
      (ref: ResourceRef, accept: (version?: ResourceVersion) => void) => {
        const key = subscriptionKey(ref);
        const set = listeners.get(key) ?? new Set();
        listeners.set(key, set);
        set.add(accept);
        return () => {
          set.delete(accept);
        };
      },
    ),
    watchResourceConnection: (
      accept: Parameters<
        import("./store.ts").SubscriptionTransport["watchResourceConnection"]
      >[0],
    ) => {
      connections.add(accept);
      accept("live", "open", {});
      return () => {
        connections.delete(accept);
      };
    },
  };
  const subscriptions = createSubscriptions(
    transport,
    createDataObserver(
      () => ({ subscribeTranscriptProjection: () => () => {} }),
      transport,
      undefined,
      recovery
        ? {
            get: vi
              .fn()
              .mockImplementation(
                (_path: string, options: { query: { request_id: string } }) =>
                  recovery.authProgress(options.query.request_id),
              ),
            post: vi
              .fn()
              .mockImplementation(
                (_path: string, body: { agent: string; after: number }) =>
                  recovery.voice(body.agent, body.after),
              ),
          }
        : undefined,
    ),
  );
  const store = createServerStore(subscriptions);
  return {
    store,
    connection(status: import("../connectionState.ts").ConnectionHealth) {
      for (const accept of connections) accept(status, "open", {});
    },
    subscriptions,
    transport,
    emit(ref: ResourceRef, version: ResourceVersion) {
      for (const accept of listeners.get(subscriptionKey(ref)) ?? [])
        accept(version);
    },
    close() {
      store.dispose();
      subscriptions.dispose();
    },
  };
}
