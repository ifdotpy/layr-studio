// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, vi } from "vitest";
import { createServerStore } from "../../store/index.ts";
import { createSubscriptions } from "../../sync/subscriptions/module.ts";
import type { ResourceVersion } from "../../sync/resourceEvents.ts";
import { createStateMirror } from "../../generated/observableResources.ts";
import {
  createSidebarViewModel,
  createSidebarRow,
  localSidebarIdentity,
  type SidebarAgent,
} from "../sidebar/viewModel.ts";
import type { components } from "../../generated/api.ts";

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: () => true,
  }));
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

export type Summary = components["schemas"]["AgentSummaryEntityDto"];

const noop = () => {};
export const actions = {
  prepare: noop,
  open: noop,
  pin: noop,
  rename: noop,
  name: noop,
  cancelRename: noop,
  beginRename: noop,
  unread: noop,
  move: noop,
  changeProject: noop,
  archive: noop,
  remove: noop,
};
function fixture() {
  let accept: (v?: ResourceVersion) => void = noop;
  const removed = vi.fn();
  const added = vi.fn((_ref, callback) => {
    accept = callback;
    return removed;
  });
  const subscriptions = createSubscriptions({
    watchResourceChanges: added,
    watchResourceConnection: (callback) => {
      callback("live", "open", {});
      return noop;
    },
  });
  const store = createServerStore(subscriptions);
  const empty = createStateMirror();
  const model = createSidebarViewModel(
    () => {
      const value = store.state().current;
      return value.status === "live" || value.status === "stale"
        ? value.data
        : empty.current;
    },
    () => ({ query: "", archived: false, opened: null, rank: () => 0 }),
  );
  let revision = 0;
  const publish = (rows: readonly Summary[]) => {
    accept({
      epoch: "fixture",
      revision: ++revision,
      cursor: { workspaceId: "workspace", seq: revision },
      replyKind: "delta",
    });
    subscriptions.publish(
      { kind: "state" },
      rows.map(
        (
          agent,
        ): Extract<
          components["schemas"]["SyncEntityPayload"],
          { collection: "agentSummary" }
        > => ({ collection: "agentSummary", id: agent.id, value: agent }),
      ),
    );
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const close = async () => {
    await act(async () => root.unmount());
    store.dispose();
    subscriptions.dispose();
  };
  return { store, model, subscriptions, added, removed, root, publish, close };
}

export { fixture };

export function makeRow(agent: SidebarAgent) {
  return createSidebarRow(agent, {
    collections: createStateMirror().current,
    ...localSidebarIdentity,
  });
}
