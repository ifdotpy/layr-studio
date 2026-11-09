import { setStartupFixture } from "../../components/shell/integration-support/observableFixture.tsx";
import { CreationOwner } from "../projects/CreationOwner.ts";
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
  const ensureConversation = vi
    .spyOn(CreationOwner.prototype, "ensureConversation")
    .mockResolvedValue("created");
  const areas = conversationActionsFixture(() => {}, models.store);
  const services = areas.scope.services;
  let text: Record<string, string> = {};
  const listeners = new Map<string, Set<() => void>>();
  const drafts = {
    getDraft: (id: string) => text[id] || "",
    subscribeDraft(id: string, listener: () => void) {
      const group = listeners.get(id) || new Set<() => void>();
      group.add(listener);
      listeners.set(id, group);
      return () => {
        group.delete(listener);
      };
    },
    setDrafts(
      value:
        | Record<string, string>
        | ((old: Record<string, string>) => Record<string, string>),
    ) {
      const old = text;
      text = typeof value === "function" ? value(old) : value;
      for (const [id, callbacks] of listeners)
        if (old[id] !== text[id]) callbacks.forEach((callback) => callback());
    },
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
