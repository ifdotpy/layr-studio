import { useEffect, useMemo, useState, type ReactNode } from "react";
import { ServerAreas } from "../../src/app/ServerAreasProvider.tsx";
import { ServerAreasContextProvider as Provider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  createServerViewServices,
  useServerApi,
  useServerStore,
} from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import BackgroundTasks from "../../src/components/shell/tasks/BackgroundTasks/BackgroundTasks.tsx";
import {
  createBackgroundTaskArea,
  type BackgroundTaskProps,
} from "../../src/components/shell/tasks/support/backgroundTaskActions.tsx";

/** Use the browser fixture's real API and subscription owner with its explicit store. */
export function browserAreas(store: ReturnType<typeof useServerStore>) {
  const services = createServerViewServices((scope) => new ServerAreas(scope));
  const areas = new ServerAreas({
    services,
    store,
    screen: { active: true, channel: null },
  });
  window.addEventListener(
    "pagehide",
    () => {
      areas.dispose();
      services.close();
    },
    { once: true },
  );
  return areas;
}

export function FixtureAreas({
  children,
  selectedId,
  notify,
}: {
  children: ReactNode;
  selectedId?: string;
  notify?: (message: string) => void;
}) {
  const store = useServerStore();
  const [areas] = useState(() => browserAreas(store));
  useEffect(
    () => areas.ui.terminal.start(null, areas.ui.createTerminal),
    [areas],
  );
  useEffect(
    () => () => {
      areas.dispose();
      areas.scope.services.close();
    },
    [areas],
  );
  useEffect(() => {
    if (notify) areas.notices.notify = notify;
    areas.navigation.selectedId = selectedId ?? null;
  }, [areas, selectedId, notify]);
  return <Provider value={areas}>{children}</Provider>;
}

/** The fixture owns panel lifetime and supplies the same actions as the shell owner. */
export function BackgroundTasksFixture(props: BackgroundTaskProps) {
  const api = useServerApi();
  const store = useServerStore();
  const actions = useMemo(
    () => createBackgroundTaskArea(api, store, "local", props),
    [api, store, props.leadId],
  );
  useEffect(() => actions.start(), [actions]);
  useEffect(() => {
    if (props.initialFocus) actions.focus(props.initialFocus.id);
  }, [actions, props.initialFocus]);
  return <BackgroundTasks {...props} actions={actions} />;
}

/** Project form request owners are created by the fixture entrypoint. */
export function browserProjectSaves(saved: () => Promise<void>) {
  return {
    account: new ProjectSave(localSidebarBackend, "/api/projects", saved),
    worker: new ProjectSave(localSidebarBackend, "/api/projects", saved),
    order: new ProjectSave(localSidebarBackend, "/api/projects", saved),
    defaultOrder: new ProjectSave(
      localSidebarBackend,
      "/api/accounts/default-groups",
      saved,
    ),
  };
}
import { ProjectSave } from "../../src/app/projects/ProjectSave.ts";
import { localSidebarBackend } from "../../src/components/sidebar/support/services.tsx";
