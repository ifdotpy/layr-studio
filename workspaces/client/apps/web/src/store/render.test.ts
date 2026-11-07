// @vitest-environment happy-dom
import { act, createElement, StrictMode, Suspense } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createServerStore } from "./index.ts";
import { createSubscriptions } from "../sync/subscriptions/module.ts";
import { sidebarRowFixture } from "../servers/sidebarRowFixture.ts";
import SidebarRow from "../components/SidebarRow.tsx";
import type { ResourceVersion } from "../sync/resourceEvents.ts";
import type { SidebarChatRow } from "../generated/observableResources.ts";

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({matches:false, media:"", onchange:null, addListener:vi.fn(), removeListener:vi.fn(), addEventListener:vi.fn(), removeEventListener:vi.fn(), dispatchEvent:()=>true}));
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

const noop = () => {};
const actions = {prepare:noop, open:noop, pin:noop, rename:noop, name:noop, cancelRename:noop, beginRename:noop, unread:noop, move:noop, changeProject:noop, archive:noop, remove:noop};
const bindings = {};
function fixture() {
  let accept: (v?: ResourceVersion) => void = noop;
  const removed = vi.fn();
  const added = vi.fn((_ref, callback) => { accept = callback; return removed; });
  const subscriptions = createSubscriptions({
    watchResourceChanges: added,
    watchResourceConnection: (callback) => { callback("live", "open", {}); return noop; },
  });
  const store = createServerStore(subscriptions);
  let revision = 0;
  const publish = (rows: readonly SidebarChatRow[]) => {
    accept({epoch:"fixture",revision:++revision,cursor:{workspaceId:"workspace",seq:revision},replyKind:"delta"});
    subscriptions.publish({kind:"sidebar"}, {rows});
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const close = async () => {
    await act(async () => root.unmount());
    store.dispose(); subscriptions.dispose();
  };
  return {store, subscriptions, added, removed, root, publish, close};
}

it.each([false, true])("real sidebar rows isolate title/status updates and equal replay (StrictMode=%s)", async (strict) => {
  const f = fixture();
  const renders = new Map<string,number>();
  window.__studioPromptComposerRenderProbe = (kind,id) => {
    const key = `${kind}:${id || ""}`;
    renders.set(key,(renders.get(key)||0)+1);
  };
  let conversationRenders = 0;
  function Conversation() { conversationRenders++; return createElement("p",null,"Conversation"); }
  const Rows = observer(function Rows() {
    const value = f.store.sidebar().current;
    if (value.status !== "live" && value.status !== "stale") return null;
    return value.data.rows.map((row) => createElement(SidebarRow,{key:row.id,row,selected:false,serverAlias:"Server",showServer:false,renaming:false,organizing:false,name:"",compact:false,markingRead:false,bindings,actions}));
  });
  const children = createElement(MantineProvider,null,createElement(Rows),createElement(Conversation));
  await act(async () => f.root.render(strict ? createElement(StrictMode,null,children) : children));
  await act(async () => vi.advanceTimersByTimeAsync(100));
  const rows = [sidebarRowFixture("a"),sidebarRowFixture("b"),sidebarRowFixture("c")];
  await act(async () => {f.publish(rows); await vi.advanceTimersByTimeAsync(0);});
  renders.clear();
  const beforeConversation = conversationRenders;
  await act(async () => {f.publish(rows.map((row)=>({...row}))); await vi.advanceTimersByTimeAsync(0);});
  expect([...renders.values()].reduce((a,b)=>a+b,0)).toBe(0);
  await act(async () => {
    f.publish(rows.map((row)=>row.id === "b" ? {...row,name:"Changed",indicator:{kind:"working",label:"Working"}} : row));
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(renders.get("sidebar-row:b")).toBeGreaterThan(0);
  expect(renders.get("sidebar-row:a") || 0).toBe(0);
  expect(renders.get("sidebar-row:c") || 0).toBe(0);
  expect(conversationRenders).toBe(beforeConversation);
  expect(f.added).toHaveBeenCalledTimes(1);
  delete window.__studioPromptComposerRenderProbe;
  await f.close();
});

it("releases an abandoned Suspense render at the pinned finalization backstop, then existing grace", async () => {
  const f = fixture();
  let resume = noop;
  const pending = new Promise<void>((resolve) => {resume=resolve;});
  const Abandoned = observer(function Abandoned() {
    void f.store.sidebar().current;
    throw pending;
  });
  await act(async () => f.root.render(createElement(Suspense,{fallback:createElement("p",null,"Loading")},createElement(Abandoned))));
  await act(async () => vi.advanceTimersByTimeAsync(100));
  expect(f.added).toHaveBeenCalledTimes(1);
  await act(async () => f.root.render(createElement("p",null,"Elsewhere")));
  await act(async () => {f.publish([sidebarRowFixture("a")]); await vi.advanceTimersByTimeAsync(0);});
  expect(f.store.sidebar().current.status).toBe("live");
  await act(async () => vi.advanceTimersByTimeAsync(10_000));
  expect(f.store.sidebar().current.status).toBe("stale");
  expect(f.removed).not.toHaveBeenCalled();
  await act(async () => vi.advanceTimersByTimeAsync(20_000));
  expect(f.removed).toHaveBeenCalledTimes(1);
  resume();
  await f.close();
});
