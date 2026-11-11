import { sidebarFixtureAreas } from "../components/sidebar/Sidebar/Sidebar.fixture.tsx";
import { localSidebarServices } from "../components/sidebar/support/services.tsx";
import { observable, action } from "mobx";
import type { CombinedSidebarServices } from "./combinedSidebarServices.ts";
import {
  CombinedSidebarOwner,
  type CombinedSidebarShell,
} from "./CombinedServerSidebar.tsx";
import { observableFixture } from "../components/shell/integration-support/observableFixture.tsx";
import Sidebar from "../components/sidebar/Sidebar/Sidebar.tsx";
import { defaultStudioPreferences } from "../studioPreferences.ts";
import type { MergedSidebar } from "./mergedSidebar.ts";
import type { ServerNavigation } from "./navigation.ts";

export type CombinedSidebarFixtureInput = {
  model: MergedSidebar;
  current: string;
  navigation: Record<string, ServerNavigation>;
  mobile: boolean;
  hidden: boolean;
  onCompactChange?: (compact: boolean) => void;
  close(): void;
  notice: string;
  hideOldChatsThreshold: number;
};
/** The fixture entrypoint owns construction, updates and teardown outside render. */
export function createCombinedSidebarFixture(
  input: CombinedSidebarFixtureInput,
  services: CombinedSidebarServices,
) {
  const facts = observable.box({ input, services }, { deep: false });

  const models = observableFixture();
  const compact = observable.box<boolean | undefined>();
  const shell: CombinedSidebarShell = {
    get model() {
      return facts.get().input.model;
    },
    get current() {
      return facts.get().input.current;
    },
    get mobileOpen() {
      return facts.get().input.mobile;
    },
    get desktopCollapsed() {
      return facts.get().input.hidden;
    },
    get isCompact() {
      return compact.get() ?? facts.get().input.mobile;
    },
    get selectedId() {
      const p = facts.get().input,
        id = p.navigation[p.current]?.opened;
      return id ? p.model.chatKey(p.current, id) : null;
    },
    navigationIntent: 0,
    get value() {
      return {
        ...defaultStudioPreferences,
        hideOldChatsThreshold: facts.get().input.hideOldChatsThreshold,
      };
    },
    recordCompact(value) {
      compact.set(value);
      facts.get().input.onCompactChange?.(value);
    },
    close() {
      facts.get().input.close();
    },
  };
  const owner = new CombinedSidebarOwner(
    shell,
    {
      get backends() {
        return facts.get().services.backends;
      },
      get aliases() {
        return facts.get().services.aliases;
      },
      online: (id) => facts.get().services.online(id),
      refreshOwner: (id) => facts.get().services.refreshOwner(id),
      executeCommand: (id, command) =>
        facts.get().services.executeCommand(id, command),
      send: (id, command) => facts.get().services.send(id, command),
      applyNavigation: (id, value) =>
        facts.get().services.applyNavigation(id, value),
      settings: () => facts.get().services.settings(),
      search: () => facts.get().services.search(),
      notify: (message) => facts.get().services.notify(message),
    },
    {
      areas: sidebarFixtureAreas(models.store, localSidebarServices),
      creation: {
        isCreating: false,
        unreplicatedCreatedAgents: [],
        lookup: () => undefined,
      },
    },
  );

  return {
    owner,
    update: action(
      (input: CombinedSidebarFixtureInput, services: CombinedSidebarServices) =>
        facts.set({ input, services }),
    ),
    start() {
      const stop = owner.start();
      return () => {
        stop();
        models.store.dispose();
      };
    },
  };
}
export function CombinedSidebarFixtureView({
  fixture,
  notice = "",
}: {
  fixture: ReturnType<typeof createCombinedSidebarFixture>;
  notice?: string;
}) {
  return <Sidebar owner={fixture.owner.list} notice={notice} />;
}
