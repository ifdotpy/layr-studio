import { ServerStoreProvider } from "../../src/servers/ServerServicesContext";
import { createServerStore } from "../../src/store";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events";
import {
  ConversationProvider,
  type ConversationActions,
} from "../../src/components/conversation/navigation.ts";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../src/components/observableFixture.tsx";
import React, {
  useCallback,
  useRef,
  useState,
  useMemo,
  useEffect,
} from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "../../src/style.css";
import { LiveConversation as Conversation } from "../../src/components/Conversation.tsx";
import { sidebarEntities } from "../../src/servers/sidebarSnapshot.ts";

import type { Agent } from "../../src/types.ts";
import type { components } from "../../src/generated/api.ts";
import type { OutgoingMessage } from "../../src/sync/send.ts";
import { fixtureSnapshot } from "./snapshot.ts";

/** JSON body authored by this fixture, excluding the local persistence callback. */
export type UxChatFixtureSendBody = { text: string } & Omit<
  NonNullable<Parameters<ConversationActions["send"]>[0]>,
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

function Fixture() {
  const [id, setId] = useState("lead");
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const currentDrafts = useRef(drafts);
  const draftListeners = useRef(new Map<string, Set<() => void>>());
  const [notice, setNotice] = useState("");
  const [jumpTarget, setJumpTarget] = useState<{
    messageId: string;
    requestId: string;
  }>();
  const [sending, setSending] = useState(false);
  const [failedOutgoing, setFailedOutgoing] = useState<OutgoingMessage[]>([]);
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
  // This partial fixture omits account groups and deletion/disconnection capabilities.
  const fixtureAccountsData: Partial<
    NonNullable<ConversationActions["accounts"]["data"]>
  > = {
    accounts: [],
    archivedAccounts: [],
    defaultAccountKey: "default",
    logins: [],
  };
  const accountsState: ConversationActions["accounts"] = {
    data: fixtureAccountsData as NonNullable<
      ConversationActions["accounts"]["data"]
    >,
    setData: () => {},
    error: "",
    refresh: async () => null,
    scope: "ux-chat-fixture",
  };
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
  const getDraft = useCallback(
    (session: string) => currentDrafts.current[session] || "",
    [],
  );
  const subscribeDraft = useCallback(
    (session: string, listener: () => void) => {
      let listeners = draftListeners.current.get(session);
      if (!listeners)
        draftListeners.current.set(session, (listeners = new Set()));
      listeners.add(listener);
      return () => {
        listeners?.delete(listener);
        if (!listeners?.size) draftListeners.current.delete(session);
      };
    },
    [],
  );
  const setDraft = (
    value: string | ((text: string) => string),
    target = id,
  ) => {
    const previous = currentDrafts.current[target] || "";
    const text = typeof value === "function" ? value(previous) : value;
    currentDrafts.current = { ...currentDrafts.current, [target]: text };
    setDrafts(currentDrafts.current);
    if (text !== previous)
      draftListeners.current.get(target)?.forEach((listener) => listener());
  };
  window.chatFixture = {
    select: setId,
    jump: (messageId: string) =>
      setJumpTarget({ messageId, requestId: crypto.randomUUID() }),
    drafts,
    id,
    branchSelections: branchSelections.current,
    regularSelections: regularSelections.current,
  };
  return (
    <ServerStoreProvider value={store}>
      <ConversationProvider
        value={{
          ...conversationActionsFixture((target) => {
            regularSelections.current.push(target);
            if (data.threads.some((thread) => thread.id === target))
              setId(target);
          }),
          accounts: accountsState,
          getDraft,
          subscribeDraft,
          setDraft,
          notify: setNotice,
          branchCreated: (target) => {
            branchSelections.current.push(target);
            setId(target);
          },
          send: async (options) => {
            setSending(true);
            try {
              const failure = window.__uxChatSendFailure;
              if (failure !== undefined) {
                const messageId = "image-refusal";
                setFailedOutgoing([
                  {
                    id: messageId,
                    body: {
                      id: messageId,
                      room: id,
                      text: getDraft(id),
                      assets: [],
                      delivery: "queue",
                    },
                    status: {
                      source: "device",
                      state: { kind: "transport_failed", error: failure },
                    },
                    created: Date.now(),
                  },
                ]);
                throw new Error(failure);
              }
              await fetch("/api/fixture-send", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                  text: getDraft(id),
                  ...options,
                } satisfies UxChatFixtureSendBody),
              });
              setDraft("");
            } finally {
              setSending(false);
            }
          },
        }}
      >
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
            <Conversation
              id={id}
              syncWorkspaceId={window.__uxChatSyncIdentity?.workspaceId}
              agent={model.mirror.current.agents.get(agent.id)}
              sending={sending}
              outgoing={failedOutgoing}

              limits={null}

              jumpTarget={jumpTarget}
            />
          </main>
        </MantineProvider>
      </ConversationProvider>
    </ServerStoreProvider>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);
