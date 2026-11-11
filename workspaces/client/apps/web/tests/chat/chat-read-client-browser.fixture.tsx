import "../../src/rendererStyles.ts";
import type { ReadStateOwner } from "../../src/app/conversation/ReadStateOwner.ts";
import { autorun } from "mobx";
import { componentSnapshot as snapshotFixture } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { observable, runInAction } from "mobx";
import { observer } from "mobx-react-lite";
import { conversationActionsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { ServerServices } from "../../src/servers/serverServices.ts";
import { createServerStore } from "../../src/store/server.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import * as api from "../../src/servers/net/api.ts";
import { defaultSyncClient } from "../../src/sync/client.ts";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import { useState, useRef } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { useVisibleChatResult } from "../../src/components/conversation/support/useChatReadState.ts";
import type { Agent, Message } from "../../src/types.ts";
import type {
  ChatReadState,
  ChatReadProof,
} from "../../src/components/conversation/support/useChatReadState.ts";
import {
  initialAgent as initial,
  readWrite,
  fixtureReadState,
} from "./chat-read-state.fixture.ts";
import type { ReadWrite } from "./chat-read-state.fixture.ts";
type Controls = ReadStateOwner;
type NativeRow = Message & { height: number; marginTop?: number };
declare global {
  interface Window {
    readWrite: typeof readWrite;
    foreground: boolean;
    visible: boolean;
    calls: (ReadWrite & { workspace: string | null })[];
    notifications: string[];
    refreshes: number;
    hold: boolean;
    fail: boolean;
    serverState: ChatReadState | null;
    release: () => void;
    releaseLost: (() => void) | null;
    transientFailure: boolean;
    lostWrites: ReadWrite[];
    lostReplies: number;
    controls: Controls;
    previousControls: Controls | null;
    agent: Agent;
    state: ChatReadState | null;
    marking: string[];
    markUnread: () => Promise<void>;
    observe: (proof: ChatReadProof) => void;
    unmount: () => void;
    setAgent: (value: Agent) => void;
    setOpened: (value: string) => void;
    setWorkspace: (value: string) => void;
    setLoaded: (value: boolean) => void;
    setMessage: (value: string) => void;
    setOffset: (value: number) => void;
    setStreaming: (value: boolean) => void;
    setPhase: (value: Message["phase"]) => void;
    replaceResult: () => void;
    setTurnStatus: (value: Message["turnStatus"]) => void;
    setLatestPage: (value: boolean) => void;
    setResultText: (value: string) => void;
    setGroupTurn: (value: string) => void;
    setGroupTurns: (value: string) => void;
    setNativeRows: (value: NativeRow[] | null) => void;
    setRowsOpen: (value: boolean) => void;
  }
}
window.readWrite = readWrite;
window.foreground = false;
window.visible = true;
window.calls = [];
window.notifications = [];
window.refreshes = 0;
window.hold = false;
window.fail = false;
window.serverState = fixtureReadState(
  localStorage.getItem("server-read-state") || "null",
);
Object.defineProperty(document, "visibilityState", {
  configurable: true,
  get: () => (window.visible ? "visible" : "hidden"),
});
document.hasFocus = () => window.foreground;

const nativeFetch = window.fetch;
window.fetch = async (input, options) => {
  const request =
    input instanceof Request ? input : new Request(input, options);
  const url = new URL(request.url);
  if (url.pathname != "/api/organization") return nativeFetch(input, options);
  const body = readWrite(JSON.parse(await request.clone().text()));
  window.calls.push({
    ...body,
    workspace: request.headers.get("X-Canvas-Workspace"),
  });
  if (window.hold)
    await new Promise<void>((resolve) => (window.release = resolve));
  if (window.fail)
    return new Response(JSON.stringify({ error: "Read state changed" }), {
      status: 409,
    });
  const value = body.read_state;
  window.serverState = {
    threadId: value.thread_id,
    turnId: value.turn_id,
    read: value.read,
    revision: value.expected_revision + 1,
  };
  localStorage.setItem("server-read-state", JSON.stringify(window.serverState));
  await nativeFetch("/api/test/read-state", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(window.serverState),
  });
  return new Response(
    JSON.stringify({
      ...initial,
      readState: window.serverState,
    }),
    { headers: { "Content-Type": "application/json" } },
  );
};
const selection = observable({ opened: "one", workspace: "first" });
const store = createServerStore(defaultSubscriptions);
const area = conversationActionsFixture(() => {}, store);
Object.defineProperty(area.scope.services, "api", {
  value: api satisfies ServerServices["api"],
});
Object.defineProperty(area.scope.services, "sync", {
  value: defaultSyncClient satisfies ServerServices["sync"],
});
setStartupFixture(area, {
  ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
  stateDir: "/state",
});
const stopSelection = autorun(() => {
  setStartupWorkspace(area, selection.workspace);
  area.navigation.selectedId = selection.opened;
});
area.notices.notify = (text) => {
  window.notifications.push(text);
};
const controls = area.read;
const stopControls = controls.start();
const Fixture = observer(function Fixture() {
  const workspace = selection.workspace;
  const [agent, setAgent] = useState<Agent>({
      ...initial,
      readState: window.serverState,
    }),
    [loaded, setLoaded] = useState(true),
    [message, setMessage] = useState("turn"),
    [offset, setOffset] = useState(900),
    [streaming, setStreaming] = useState(false),
    [phase, setPhase] = useState<Message["phase"]>("final_answer"),
    [nodeVersion, setNodeVersion] = useState(0),
    [turnStatus, setTurnStatus] = useState<Message["turnStatus"]>(undefined),
    [latestPage, setLatestPage] = useState(true),
    [resultText, setResultText] = useState("Completed result"),
    [groupTurn, setGroupTurn] = useState("turn"),
    [groupTurns, setGroupTurns] = useState("turn"),
    [nativeRows, setNativeRows] = useState<NativeRow[] | null>(null),
    [rowsOpen, setRowsOpen] = useState(true);
  const scroll = useRef<HTMLDivElement>(null);
  window.controls = controls;
  window.agent = agent;
  window.state = controls.readStateFor(agent);
  window.marking = [...controls.marking];
  window.markUnread = () => controls.markUnread(agent);
  window.setAgent = (value) => flushSync(() => setAgent(value));
  window.setOpened = (value) =>
    flushSync(() =>
      runInAction(() => {
        selection.opened = value;
      }),
    );
  window.setWorkspace = (value) =>
    flushSync(() =>
      runInAction(() => {
        selection.workspace = value;
      }),
    );
  window.setLoaded = (value) => flushSync(() => setLoaded(value));
  window.setMessage = (value) => flushSync(() => setMessage(value));
  window.setOffset = (value) => flushSync(() => setOffset(value));
  window.setStreaming = (value) => flushSync(() => setStreaming(value));
  window.setPhase = (value) => flushSync(() => setPhase(value));
  window.replaceResult = () => flushSync(() => setNodeVersion((v) => v + 1));
  window.setTurnStatus = (value) => flushSync(() => setTurnStatus(value));
  window.setLatestPage = (value) => flushSync(() => setLatestPage(value));
  window.setResultText = (value) => flushSync(() => setResultText(value));
  window.setGroupTurn = (value) => flushSync(() => setGroupTurn(value));
  window.setGroupTurns = (value) => flushSync(() => setGroupTurns(value));
  window.setNativeRows = (value) => flushSync(() => setNativeRows(value));
  window.setRowsOpen = (value) => flushSync(() => setRowsOpen(value));
  window.observe = (proof) => controls.observeRead(proof);
  useVisibleChatResult(
    scroll,
    agent,
    nativeRows || [
      {
        id: "result",
        role: "assistant",
        turnId: message,
        phase,
        streaming,
        turnStatus,
        text: resultText,
      },
    ],
    loaded,
    controls.observeRead,
    workspace,
    latestPage,
  );
  return (
    <>
      <div id="messages" ref={scroll} style={{ height: 180, overflow: "auto" }}>
        <div style={{ height: offset }} />
        <section
          key={nodeVersion}
          data-turn={groupTurn}
          data-turns={groupTurns}
          data-outcome={turnStatus}
        >
          <>
            {nativeRows ? (
              <details open={rowsOpen}>
                <summary>Joined work</summary>
                {nativeRows.map((row) => (
                  <div
                    key={row.id}
                    data-message={row.id}
                    style={{
                      height: row.height,
                      marginTop: row.marginTop || 0,
                    }}
                  >
                    {row.text}
                  </div>
                ))}
              </details>
            ) : (
              <article data-message="result" style={{ height: 80 }}>
                Completed result
              </article>
            )}
          </>
        </section>
      </div>
      <button onClick={() => void controls.markUnread(agent)}>Unread</button>
    </>
  );
});
const root = document.getElementById("root")!;
const appRoot = createRoot(root);
window.unmount = () => {
  appRoot.unmount();
  stopControls();
  stopSelection();
  store.dispose();
  area.scope.services.close();
};
appRoot.render(
  <ServerStoreProvider value={store}>
    <Fixture />
  </ServerStoreProvider>,
);

import {
  setStartupFixture,
  setStartupWorkspace,
} from "../../src/components/shell/integration-support/observableFixture.tsx";
