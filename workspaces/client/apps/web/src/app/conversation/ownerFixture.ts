import { observable, action } from "mobx";
import { setStartupFixture } from "../../components/shell/integration-support/observableFixture.tsx";
import { vi } from "vitest";
import { AgentSource } from "../../generated/api.ts";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../components/shell/integration-support/observableFixture.tsx";
import { componentSnapshot as snapshotFixture } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import type { createDrafts } from "../../sync/drafts.ts";

export function ownerFixture() {
  const models = observableFixture([
    {
      id: "lead",
      name: "Lead",
      source: AgentSource.managed,
      isLead: true,
      rootId: "lead",
      threadId: "thread",
    },
  ]);
  const areas = conversationActionsFixture(() => {}, models.store);
  const ensureConversation = vi
    .spyOn(areas.creation, "ensureConversation")
    .mockResolvedValue("created");
  const services = areas.scope.services;
  const text = observable.map<string, string>();
  const drafts = {
    getDraft: (id: string) => text.get(id) || "",
    setDrafts: action(function (
      value:
        | Record<string, string>
        | ((old: Record<string, string>) => Record<string, string>),
    ) {
      text.replace(
        typeof value === "function" ? value(Object.fromEntries(text)) : value,
      );
    }),
    conflicts: [],
    dismissDraft() {},
    error: "",
    localPersistenceFailed: false,
    start: () => () => {},
  } satisfies ReturnType<typeof createDrafts>;
  const notify = vi.fn(),
    post = vi.fn();
  Object.defineProperty(services, "drafts", { value: drafts });
  services.api.post = post;
  services.sync.refreshState = vi.fn(async () => {});
  setStartupFixture(
    areas,
    Object.assign(snapshotFixture(), {
      token: "",
      stateDir: "/fixture",
    }),
  );
  areas.startup.workspaceId = "workspace";
  areas.navigation.selectedId = "lead";
  areas.layout.geometry.viewport.width = 1280;
  areas.notices.notify = notify;
  const { navigation, native, rename, read, delivery, selected } = areas;
  return {
    areas,
    services,
    models,
    navigation,
    drafts,
    notify,
    post,
    native,
    rename,
    read,
    delivery,
    selected,
    ensureConversation,
    dispose() {
      ensureConversation.mockRestore();
      services.close();
      models.store.dispose();
    },
  };
}
