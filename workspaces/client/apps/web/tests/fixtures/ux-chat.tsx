import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { runInAction, observable, action } from "mobx";
import {
  createServerServices,
  type ServerServices,
} from "../../src/servers/serverServices.ts";
import * as fixtureApi from "../../src/servers/net/api.ts";
import { createServerStore } from "../../src/store/server.ts";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import {
  ConversationProvider,
  type ConversationActions,
} from "../../src/components/conversation/support/navigation.ts";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../src/components/shell/integration-support/observableFixture.tsx";
import React, { useRef, useState, useMemo, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "../../src/style.css";
import Conversation from "../../src/components/conversation/ObservedConversation/ObservedConversation.tsx";
import { sidebarEntities } from "../../src/servers/sidebarSnapshot.ts";

import type { Agent } from "../../src/types.ts";
import type { components } from "../../src/generated/api.ts";
import { fixtureSnapshot } from "./snapshot.ts";

/** JSON body authored by this fixture, excluding the local persistence callback. */
export type UxChatFixtureSendBody = { text: string } & Omit<
  NonNullable<Parameters<ConversationActions["delivery"]["send"]>[0]>,
  "onPersist"
>;

declare global {
  interface Window {
    __uxChatSyncIdentity?: components["schemas"]["SyncIdentityResponse"];
    __uxChatSendFailure?: string;
    chatFixture: {
      select: (id: string) => void;
      jump: (messageId: string) => void;
      drafts: Record<string, string>;
      id: string;
      branchSelections: string[];
      regularSelections: string[];
    };
  }
}

const draftValues = observable.map<string, string>();
function Fixture() {
  const [id, setId] = useState("lead");
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [notice, setNotice] = useState("");
  const [jumpTarget, setJumpTarget] = useState<{
    messageId: string;
    requestId: string;
  }>();
  const branchSelections = useRef<string[]>([]);
  const regularSelections = useRef<string[]>([]);
  const agent = {
    id,
    rootId: id,
    isLead: false,
    source: "managed",
    name: id,
    cwd: "/fixture",
    status: "idle",
    canSend: true,
    activity: { phase: "idle" },
  } satisfies Agent;
  const data = fixtureSnapshot([agent]);
  const model = useMemo(() => observableFixture([agent]), [id]);
  const [runtime] = useState(() => createServerStore(defaultSubscriptions));
  const store = useMemo(
    () => ({
      ...runtime,
      state: model.store.state,
      agentDetail: model.store.agentDetail,
    }),
    [runtime, model],
  );
  useEffect(() => () => runtime.dispose(), [runtime]);
  useEffect(() => () => model.store.dispose(), [model]);
  model.apply([
    ...sidebarEntities(model.mirror.current).filter(
      (entity) => entity.collection !== "workspace",
    ),
    {
      collection: "workspace",
      id: "workspace",
      value: { stateDir: data.stateDir },
    },
  ]);
  const getDraft = (session: string) => draftValues.get(session) || "";
  window.chatFixture = {
    select: setId,
    jump: (messageId: string) =>
      setJumpTarget({ messageId, requestId: crypto.randomUUID() }),
    drafts,
    id,
    branchSelections: branchSelections.current,
    regularSelections: regularSelections.current,
  };
  const [services] = useState(() => {
    const value = createServerServices({
      id: "local",
      origin: window.location.origin,
      label: "Fixture",
    });
    Object.defineProperty(value, "api", {
      value: fixtureApi satisfies ServerServices["api"],
    });
    Object.defineProperty(value, "resources", {
      value: defaultSubscriptions satisfies ServerServices["resources"],
    });
    return value;
  });
  useEffect(() => () => services.close(), [services]);
  const area = useMemo(
    () =>
      conversationActionsFixture(
        (target) => {
          if (data.threads.some((thread) => thread.id === target))
            regularSelections.current.push(target);
          else branchSelections.current.push(target);
          setId(target);
        },
        store,
        services,
      ),
    [store, services],
  );
  area.scope.services.drafts.getDraft = getDraft;
  runInAction(() => {
    setStartupFixture(area, data);
    area.startup.workspaceId =
      window.__uxChatSyncIdentity?.workspaceId || "workspace";
    area.navigation.selectedId = id;
    area.navigation.jumpTarget = jumpTarget
      ? { ...jumpTarget, chatId: id }
      : undefined;
  });
  const selected = area.selected;
  useEffect(() => selected.start(), [selected]);
  useEffect(() => area.layout.start(), [area]);
  services.drafts.setDrafts = (update) => {
    const previous = Object.fromEntries(draftValues);
    const next = typeof update === "function" ? update(previous) : update;
    action(() => draftValues.replace(next))();
    setDrafts(next);
  };
  services.outbox.durableSend = async (body, _attachments, persist) => {
    const failure = window.__uxChatSendFailure;
    if (failure !== undefined) throw new TypeError(failure);
    await fetch("/api/fixture-send", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    await persist?.();
    return { kind: "confirmed", id: body.id };
  };
  area.notices.notify = setNotice;
  return (
    <ServerStoreProvider value={store}>
      <ConversationProvider value={area}>
        <MantineProvider defaultColorScheme="dark">
          <main
            style={{
              height: "100dvh",
              display: "flex",
              flexDirection: "column",
            }}
          >
            {notice && (
              <div id="fixture-notice" role="status">
                {notice}
              </div>
            )}
            <Conversation />
          </main>
        </MantineProvider>
      </ConversationProvider>
    </ServerStoreProvider>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);

import { setStartupFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
