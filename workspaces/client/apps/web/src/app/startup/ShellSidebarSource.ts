import type { ServerServices } from "../../servers/serverServices.ts";
import { sidebarCollectionView } from "../../servers/sidebarSnapshot.ts";
import { createSidebarViewModel } from "../../components/sidebar/viewModel.ts";
import { makeAutoObservable, observableRef } from "mobx";
import { layoutQueries } from "../../layout/layout-contract.generated.ts";
import { createMergedSidebarSelector } from "../../servers/mergedSidebar.ts";
import type { SidebarSource } from "../../servers/mergedSidebar.ts";
import type { ServerNavigation } from "../../servers/navigation.ts";
import type { ConnectionHealth } from "../../sync/connectionState.ts";
import {
  defaultStudioPreferences,
  parseStudioPreferences,
  studioPreferencesStorageKey,
  type StudioPreferences,
} from "../../studioPreferences.ts";
import { preferenceMemory } from "../../servers/storage.ts";
import { serverSidebarBackend } from "../../servers/serverSidebarBackend.ts";
import type { ScreenSelection } from "./ScreenOwner.ts";

type Edit<T> = T | ((previous: T) => T);
/** Canonical shell inputs change at navigation, storage and visibility event sources. */
export class ShellSidebarSource {
  navigation: Record<string, ServerNavigation> = {};
  status: Record<string, ConnectionHealth> = {};
  mobileOpen = window.matchMedia(layoutQueries.navigationCompact).matches;
  compact = this.mobileOpen;
  hidden = false;
  value: StudioPreferences;
  readonly selection: ScreenSelection;
  readonly servicesFor: (owner: string) => ServerServices | undefined;
  readonly select = createMergedSidebarSelector();
  constructor(
    selection: ScreenSelection,
    servicesFor: (owner: string) => ServerServices | undefined,
  ) {
    this.selection = selection;
    this.servicesFor = servicesFor;
    try {
      this.value = parseStudioPreferences(
        preferenceMemory.getItem(studioPreferencesStorageKey) || "null",
      );
    } catch {
      this.value = defaultStudioPreferences;
    }
    makeAutoObservable(
      this,
      {
        selection: false,
        servicesFor: false,
        select: false,
        navigation: observableRef,
        status: observableRef,
        value: observableRef,
      },
      { autoBind: true },
    );
  }
  get current() {
    return this.selection.current;
  }
  get selectedId() {
    const id = this.navigation[this.current]?.opened;
    return id ? this.model.chatKey(this.current, id) : null;
  }
  get navigationIntent() {
    return this.selection.intent;
  }
  get isCompact() {
    return this.compact;
  }
  get desktopCollapsed() {
    return this.hidden;
  }
  get backends() {
    return new Map(
      this.selection.available.flatMap((id) => {
        const service = this.servicesFor(id);
        return service
          ? [
              [
                id,
                serverSidebarBackend(service, () => this.online(id)),
              ] as const,
            ]
          : [];
      }),
    );
  }
  get model() {
    return this.select(this.sources);
  }
  get sources() {
    return this.selection.available.flatMap((id): SidebarSource[] => {
      const service = this.servicesFor(id);
      const state = service?.store.state().current;
      const sidebar = this.navigation[id]?.sidebar;
      if (
        !service ||
        !sidebar ||
        !state ||
        (state.status !== "live" && state.status !== "stale")
      )
        return [];
      const collections = state.data;
      return [
        {
          id,
          online: this.online(id),
          creation: service.areas.creation,
          sidebar: sidebarCollectionView(sidebar, collections),
          collections,
          view: createSidebarViewModel(
            () => collections,
            () => ({
              query: "",
              archived: false,
              opened: null,
              rank: () => 0,
            }),
          ),
        },
      ];
    });
  }
  online(owner: string) {
    return (
      this.selection.available.includes(owner) && this.status[owner] === "live"
    );
  }
  setNavigation(edit: Edit<typeof this.navigation>) {
    const next = typeof edit === "function" ? edit(this.navigation) : edit;
    this.navigation = next;
  }
  setStatus(edit: Edit<typeof this.status>) {
    this.status = typeof edit === "function" ? edit(this.status) : edit;
  }
  setPreferences(value: StudioPreferences) {
    this.value = value;
  }
  setMobileOpen(edit: Edit<boolean>) {
    this.mobileOpen = typeof edit === "function" ? edit(this.mobileOpen) : edit;
  }
  setHidden(edit: Edit<boolean>) {
    this.hidden = typeof edit === "function" ? edit(this.hidden) : edit;
  }
  recordCompact(value: boolean) {
    this.compact = value;
  }
  close() {
    this.mobileOpen = false;
    if (!this.compact) this.hidden = true;
  }
}
