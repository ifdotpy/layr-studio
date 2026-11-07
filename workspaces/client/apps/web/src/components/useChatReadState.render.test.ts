import { WorkerIndicator } from "./agents/worker/Heading.tsx";
import TeamFixture from "./agents/TeamFixture.tsx";
import { observableFixture } from "./observableFixture.tsx";
// @vitest-environment happy-dom
import { act, createElement, useMemo } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { useChatReadState } from "./useChatReadState.ts";
import {
  agentIndicators,
  formatAgentStatus,
} from "../status/operationalStatus.ts";
import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
import { snapshotFixture } from "../snapshotFixture.ts";
const service = vi.hoisted(() => ({
  store: null as
    | ReturnType<
        typeof import("./observableFixture.tsx").observableFixture
      >["store"]
    | null,
  post: vi.fn(),
  refreshProjection: vi.fn(),
  subscribeStateProjection: () => () => {},
}));
vi.mock("../servers/ServerServicesContext", () => ({
  useServerStore: () => service.store,
  useServerApi: () => ({ post: service.post }),
  useServerSync: () => service,
}));
vi.mock("../sync/client", () => ({}));
let root: Root | undefined;
afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  root = undefined;
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
it("keeps the memoized conversation and sidebar aligned during a deferred read CAS", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  const proof = {
    threadId: "thread",
    turnId: "turn",
    read: false,
    revision: 1,
  };
  const agent = agentSummaryFixture("lead", {
    threadId: proof.threadId,
    lastCompletedTurn: proof.turnId,
    lastCompletedTurnStatus: "completed",
    readState: proof,
    markRead: proof,
    operationalStatus: { reason: "unread_result" },
  });
  const data = snapshotFixture([agent]);
  service.store = observableFixture([agent]).store;
  let finish: (response: unknown) => void = () => {};
  service.post.mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  const notify = vi.fn(),
    refresh = async () => {};
  function Caller() {
    const read = useChatReadState(data, agent.id, notify, refresh, "workspace");
    // This is App's memoized caller: facts stay unchanged while the CAS is pending.
    const indicators = useMemo(
      () => agentIndicators(data.threads, read.pendingReadTargets),
      [data.threads, read.pendingReadTargets, read.readStateFor],
    );
    return createElement(
      TeamFixture,
      { leadId: agent.id, pendingReads: read.pendingReadTargets },
      createElement(
        "div",
        null,
        createElement(WorkerIndicator, { agent }),
        createElement(
          "button",
          { onClick: () => read.select(agent.id) },
          "Read",
        ),
        createElement("span", {
          "data-conversation": indicators.get(agent.id)?.kind,
        }),
        createElement("span", {
          "data-sidebar": formatAgentStatus(
            agent,
            read.pendingReadTargets.get(agent.id),
          ).kind,
        }),
      ),
    );
  }
  const host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root?.render(createElement(Caller)));
  expect(
    host
      .querySelector("[data-conversation]")
      ?.getAttribute("data-conversation"),
  ).toBe("unread");
  expect(host.querySelector(".chat-status-unread")).not.toBeNull();
  await act(async () => host.querySelector("button")?.click());
  expect(service.post).toHaveBeenCalledTimes(1);
  expect(host.querySelector(".chat-status-unread")).toBeNull();
  expect(
    host.querySelector("[data-sidebar]")?.getAttribute("data-sidebar"),
  ).toBe("none");
  expect(
    host
      .querySelector("[data-conversation]")
      ?.getAttribute("data-conversation"),
  ).toBe("none");
  await act(async () =>
    finish({ id: agent.id, readState: { ...proof, read: true, revision: 2 } }),
  );
  expect(
    host
      .querySelector("[data-conversation]")
      ?.getAttribute("data-conversation"),
  ).toBe("unread");
  expect(
    host.querySelector("[data-sidebar]")?.getAttribute("data-sidebar"),
  ).toBe("unread");
  expect(notify).not.toHaveBeenCalled();
});
