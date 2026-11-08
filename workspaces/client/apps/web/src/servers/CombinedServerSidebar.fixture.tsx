import { sidebarFixtureAreas } from "../components/Sidebar.fixture.tsx";
import { localSidebarServices } from "../components/sidebar/services.tsx";
import { useEffect, useState } from "react";
import { observable, runInAction } from "mobx";
import { useCombinedSidebarServices } from "./combinedSidebarServices.ts";
import {
  CombinedSidebarOwner,
  type CombinedSidebarShell,
} from "./CombinedServerSidebar.tsx";
import { observableFixture } from "../components/observableFixture.tsx";
import Sidebar from "../components/Sidebar.tsx";
import { defaultStudioPreferences } from "../studioPreferences.ts";
import type { MergedSidebar } from "./mergedSidebar.ts";
import type { ServerNavigation } from "./navigation.ts";

/** Controlled scenarios adapt only their explicit facts; production uses the shell source. */
export function CombinedSidebarFixture(input: {
  model: MergedSidebar;
  current: string;
  navigation: Record<string, ServerNavigation>;
  mobile: boolean;
  hidden: boolean;
  onCompactChange?: (compact: boolean) => void;
  close(): void;
  notice: string;
  hideOldChatsThreshold: number;
}) {
  const services = useCombinedSidebarServices();
  const [facts] = useState(() =>
    observable.box({ input, services }, { deep: false }),
  );
  useEffect(() => {
    runInAction(() => facts.set({ input, services }));
  }, [facts, input, services]);
  const [fixture] = useState(() => {
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
    return { models, owner };
  });
  useEffect(() => {
    const stop = fixture.owner.start();
    return () => {
      stop();
      fixture.models.store.dispose();
    };
  }, [fixture]);
  return <Sidebar owner={fixture.owner.list} notice={input.notice} />;
}
