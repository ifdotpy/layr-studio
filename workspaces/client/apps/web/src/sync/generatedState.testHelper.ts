import { autorun } from "mobx";
import { createEntityDataObserver } from "./subscriptions/entityData.ts";
import { createSubscriptions } from "./subscriptions/module.ts";
import { createServerStore } from "../store/index.ts";
import { resourceData } from "../store/runtime.ts";
import {
  serializeStateCollections,
  type StateCollections,
} from "../generated/observableResources.ts";
import type { createSyncClient } from "./client.ts";

/** Observe native generated models through the production durable state lease. */
export function observeGeneratedState(
  client: Pick<
    ReturnType<typeof createSyncClient>,
    "syncDatabase" | "retainStateDelivery" | "projectionMetadata"
  >,
  accept: (state: StateCollections) => void,
  failed: (error: unknown) => void,
) {
  const subscriptions = createSubscriptions(
    {
      watchResourceChanges: () => () => {},
      watchResourceConnection: () => () => {},
    },
    createEntityDataObserver(client.syncDatabase, client.retainStateDelivery),
  );
  const store = createServerStore(subscriptions);
  const stop = autorun(() => {
    const current = store.state().current;
    if (
      current.status === "error" ||
      (current.status === "stale" && current.error)
    )
      failed(current.error);
    const data = resourceData(current);
    if (!client.projectionMetadata.ready || !data) return;
    void client.projectionMetadata.complete;
    serializeStateCollections(data);
    accept(data);
  });
  return {
    store,
    stop() {
      stop();
      store.dispose();
      subscriptions.dispose();
    },
  };
}
