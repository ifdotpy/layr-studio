import { runInAction } from "mobx";
import "@mantine/core/styles.css";
import { ServerAreas } from "../../../app/ServerAreasProvider.tsx";
import {
  createServerServices,
  type ServerServices,
} from "../../../servers/serverServices.ts";
import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import { ExecutionSettings } from "../../agents/ExecutionSettings/ExecutionSettings.tsx";
import type { ComponentProps } from "react";
import { useEffect, useLayoutEffect, useState, type ReactNode } from "react";
import {
  agentId,
  createStateMirror,
  createProjectionMetadataMirror,
  serializeStateCollections,
  type StateCollections,
  createAgentDetailMirror,
  createProcessesMirror,
  type AgentId,
} from "../../../generated/observableResources.ts";

import type { Agent } from "../../../types.ts";
import type { SidebarAgent } from "../../sidebar/support/viewModel.ts";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import {
  ServerStoreProvider,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createServerStore } from "../../../store/server.ts";
import { createSubscriptions } from "../../../sync/subscriptions/module.ts";

/** Keeps controlled browser/story facts in their private generated models. */
export function AgentModelsFixture({
  agents,
  children,
}: {
  agents: readonly Agent[];
  children: ReactNode;
}) {
  const [models] = useState(() => observableFixture(agents));
  useLayoutEffect(() => {
    for (const agent of agents) {
      const detail = models.details.get(agent.id);
      if (detail) detail.apply(agent);
      else models.details.set(agent.id, createAgentDetailMirror(agent));
    }
    models.apply(agentEntities(agents));
  }, [agents, models]);
  useEffect(() => () => models.store.dispose(), [models]);
  return <models.Provider>{children}</models.Provider>;
}

function agentEntities(agents: readonly Agent[]) {
  return agents.map(
    (
      agent,
    ): { collection: "agentSummary"; id: string; value: SidebarAgent } => ({
      collection: "agentSummary",
      id: agent.id,
      value: {
        ...agentSummaryFixture(agent.id),
        ...agent,
        inFlight: agent.inFlight === true,
        isLead: agent.isLead === true,
        projectServerId: agent.projectServerId ?? null,
      },
    }),
  );
}

const fixtureMirrors = new WeakMap<
  StateCollections,
  ReturnType<typeof createStateMirror>
>();

/** Explicit generated models for isolated stories and renderer assertions. */
export function observableFixture(
  agents: readonly (Agent & Pick<SidebarAgent, "hasBackgroundWork">)[] = [],
) {
  const mirror = createStateMirror();
  fixtureMirrors.set(mirror.current, mirror);
  const details = new Map(
    agents.map((agent) => [agent.id, createAgentDetailMirror(agent)]),
  );
  mirror.apply(agentEntities(agents));
  const processes = createProcessesMirror({ processes: [], activities: [] });
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: (callback) => {
      callback("live", "open", {});
      return () => {};
    },
  });
  const runtime = createServerStore(subscriptions);
  const store: ReturnType<typeof createServerStore> = {
    ...runtime,
    state: () => ({ current: { status: "live", data: mirror.current } }),
    agentDetail: (id: AgentId) => {
      const detail = details.get(id);
      return {
        current: detail
          ? { status: "live", data: detail.current }
          : { status: "loading" },
      };
    },
    processes: () => ({ current: { status: "live", data: processes.current } }),
    dispose: () => {
      runtime.dispose();
      subscriptions.dispose();
    },
  };
  return {
    store,
    mirror,
    details,
    processes,
    apply(rows: Parameters<typeof mirror.apply>[0]) {
      mirror.apply(
        rows.some((row) => row.collection === "workspace")
          ? rows
          : [
              ...serializeStateCollections(mirror.current).filter(
                (row) => row.collection === "workspace",
              ),
              ...rows,
            ],
      );
    },
    Provider({ children }: { children?: ReactNode }) {
      return (
        <ServerStoreProvider value={store}>{children}</ServerStoreProvider>
      );
    },
  };
}

/** Construct the real composition with explicit test services, without starting it. */
export function conversationActionsFixture(
  open: (id: string) => void = () => {},
  store = observableFixture().store,
  injectedServices?: ServerServices,
): ServerAreas {
  const services =
    injectedServices ||
    createServerServices({
      id: "local",
      origin: "https://fixture.test",
      label: "Fixture",
    });
  if (!injectedServices) {
    const values = new Map<string, string>();
    const storage: Storage = {
      get length() {
        return values.size;
      },
      key: (index) => [...values.keys()][index] ?? null,
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => {
        values.set(key, value);
      },
      removeItem: (key) => {
        values.delete(key);
      },
      clear: () => values.clear(),
    };
    services.storage = storage;
    services.api.saved = (key, fallback) => {
      const value = storage.getItem(key);
      return value === null ? fallback : JSON.parse(value);
    };
    services.api.save = (key, value) =>
      storage.setItem(key, JSON.stringify(value));
    services.api.get = async () => {
      throw new Error("Unexpected fixture GET");
    };
    services.api.post = async () => {
      throw new Error("Unexpected fixture POST");
    };
    const resources: ServerServices["resources"] = createSubscriptions({
      watchResourceChanges: () => () => {},
      watchResourceConnection: (
        accept: Parameters<
          ServerServices["resources"]["watchResourceConnection"]
        >[0],
      ) => {
        accept("live", "open", {});
        return () => {};
      },
      watchTokenRateEvents: () => () => {},
      watchLiveDeltas: () => () => {},
      acknowledgeEntitySequences() {},
      replaceStream() {},
      retryNow() {},
      dispose() {},
    });
    Object.defineProperty(services, "resources", { value: resources });
    const close = services.close;
    services.close = (removeCache) => {
      resources.dispose();
      close(removeCache);
    };
  }
  const areas = new ServerAreas({
    services,
    store,
    screen: { active: true, channel: null },
  });
  areas.open = open;
  return areas;
}

/** Standalone settings fixtures use the same explicit server input as their caller. */
export const ExecutionSettingsFixture = observer(
  function ExecutionSettingsFixture({
    agent,
    lead,
    ...props
  }: Omit<ComponentProps<typeof ExecutionSettings>, "input" | "leadInput"> & {
    agent: Agent;
    lead?: Agent | undefined;
  }) {
    const store = useServerStore();
    const detail = resourceData(store.agentDetail(agentId(agent.id)).current);
    const leadDetail = lead
      ? resourceData(store.agentDetail(agentId(lead.id)).current)
      : undefined;
    return (
      <ExecutionSettings
        {...props}
        input={{ kind: "agent", summary: agent, detail }}
        leadInput={
          lead
            ? { kind: "agent", summary: lead, detail: leadDetail }
            : undefined
        }
      />
    );
  },
);

const startupMetadata = new WeakMap<
  ServerServices["sync"],
  ReturnType<typeof createProjectionMetadataMirror>
>();

/** Drive startup identity through the same native models as production. */
export function setStartupFixture<T extends { stateDir: string }>(
  areas: ServerAreas,
  data: T | null,
) {
  const sync = areas.scope.services.sync;
  const previous = sync.projectionMetadata;
  let metadata = startupMetadata.get(sync);
  if (!metadata) {
    metadata = createProjectionMetadataMirror(previous);
    startupMetadata.set(sync, metadata);
    const current = metadata;
    Object.defineProperty(sync, "projectionMetadata", {
      get: () => current.current,
      configurable: true,
    });
  }
  metadata.apply({ ...metadata.current, ready: data !== null });
  const state = resourceData(areas.scope.store.state().current);
  const mirror = state && fixtureMirrors.get(state);
  if (mirror && data)
    runInAction(() =>
      mirror.apply([
        ...serializeStateCollections(mirror.current).filter(
          (row) => row.collection !== "workspace",
        ),
        {
          collection: "workspace",
          id: "current",
          value: { stateDir: data.stateDir },
        },
      ]),
    );
}
