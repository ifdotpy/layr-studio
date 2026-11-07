import { ProcessSourceKind } from "../../generated/api";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as defaultApi from "../../api";
import { createSubscriptions } from "../../sync/subscriptions/module";
import { createServerStore } from "../../store";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture";
import type { components } from "../../generated/api";
import BackgroundTasks from "./BackgroundTasks";
import { backgroundDisplay } from "../componentTestFixtures";

const h = vi.hoisted(() => ({
  store: null as ReturnType<typeof createServerStore> | null,
  post: vi.fn(async () => ({ status: "completed", processes: [] })),
  get: vi.fn(async () => ({})),
}));
vi.mock("../../servers/ServerServicesContext", async (original) => ({
  ...(await original<typeof import("../../servers/ServerServicesContext")>()),
  useServerStore: () => h.store,
  useServerApi: () => ({ ...defaultApi, post: h.post, get: h.get }),
  useServerServices: () => null,
}));
beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent() {
      return true;
    },
  }));
  h.post.mockClear();
  h.get.mockClear();
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
  delete window.__studioBackgroundTaskRenderProbe;
});
function process(id = "command"): components["schemas"]["ProcessView"] {
  return {
    id,
    agent: "lead",
    created: 2,
    status: "running",
    name: id,
    source: ProcessSourceKind.codex_task,
    key: id + "-key",
    generation: "generation",
    accountKey: "default",
    epoch: 1,
    tail: "Streamed command output",
    bytes: 23,
    cancelRequested: false,
    outputTruncated: false,
    outputPatterns: [],
    display: backgroundDisplay({
      title: id,
      actions: ["readOutput", "stop", "input"],
      stdinState: "open",
    }),
  };
}
async function fixture(records = [process()]) {
  const added = vi.fn(
    (_ref: import("../../sync/subscriptions/contracts").ResourceRef) =>
      () => {},
  );
  const subscriptions = createSubscriptions({
    watchResourceChanges: added,
    watchResourceConnection: (callback) => {
      callback("live", "open", {});
      return () => {};
    },
  });
  const store = createServerStore(subscriptions);
  h.store = store;
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const notify = vi.fn();
  const render = async (leadId = "lead") =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(BackgroundTasks, {
            opened: true,
            close() {},
            leadId,
            openAgent() {},
            refresh: async () => {},
            notify,
          }),
        ),
      ),
    );
  await render();
  await act(async () => vi.advanceTimersByTimeAsync(100));
  const publish = async (
    rows: components["schemas"]["ProcessView"][],
    responseTarget?: components["schemas"]["ProcessListing"]["responseTarget"],
  ) =>
    act(async () => {
      subscriptions.publish(
        { kind: "processes", agentId: "lead" },
        {
          processes: rows,
          activities: rows.map((value) => ({
            source: "process" as const,
            value,
          })),
          ...(responseTarget ? { responseTarget } : {}),
        },
      );
      await vi.advanceTimersByTimeAsync(0);
    });
  const owners = async (name = "Owner") =>
    act(async () => {
      subscriptions.publish({ kind: "state" }, [
        {
          collection: "agentSummary",
          id: "lead",
          value: agentSummaryFixture("lead", {
            name,
            epoch: 1,
            accountKey: "default",
          }),
        },
      ]);
      await vi.advanceTimersByTimeAsync(0);
    });
  await publish(records);
  await act(async () => vi.advanceTimersByTimeAsync(100));
  await owners();
  return {
    store,
    subscriptions,
    added,
    host,
    root,
    render,
    publish,
    owners,
    notify,
    close: async () => {
      await act(async () => root.unmount());
      store.dispose();
      subscriptions.dispose();
    },
  };
}
function button(label: string) {
  const button = [...document.querySelectorAll("button")].find(
    (b) => b.textContent === label,
  );
  expect(button).toBeDefined();
  return button!;
}
it("updates the task title from its observable model without a parent render", async () => {
  const f = await fixture();
  const renders = new Map<string, number>();
  window.__studioBackgroundTaskRenderProbe = (leaf, id) => {
    const key = leaf + ":" + (id || "");
    renders.set(key, (renders.get(key) || 0) + 1);
  };
  try {
    await f.publish([
      {
        ...process(),
        display: { ...process().display, title: "Changed command" },
      },
    ]);
    expect(document.querySelector(".task-row strong")?.textContent).toBe(
      "Changed command",
    );
    expect(renders.get("background-task-title:command")).toBe(1);
    for (const unrelated of [
      "background-task-list:",
      "background-task-row:command",
      "background-task-detail:command",
      "background-task-owner:command",
      "background-task-status:command",
      "background-task-output:command",
    ])
      expect(renders.get(unrelated) || 0, unrelated).toBe(0);
  } finally {
    await f.close();
  }
});
it("isolates tail, owner, status and clock updates from unrelated leaves", async () => {
  const f = await fixture([process(), process("other")]);
  const renders = new Map<string, number>();
  window.__studioBackgroundTaskRenderProbe = (leaf, id) => {
    const key = leaf + ":" + (id || "");
    renders.set(key, (renders.get(key) || 0) + 1);
  };
  try {
    await f.publish([{ ...process(), tail: "New output" }, process("other")]);
    expect(document.querySelector(".task-output")?.textContent).toBe(
      "New output",
    );
    expect(renders.get("background-task-output:command")).toBe(1);
    for (const leaf of [
      "background-task-title:command",
      "background-task-status:command",
      "background-task-owner:command",
      "background-task-row:command",
      "background-task-list:",
      "background-task-detail:command",
      "background-task-row:other",
    ])
      expect(renders.get(leaf) || 0, leaf).toBe(0);
    renders.clear();
    await f.owners("Renamed owner");
    expect(document.querySelector(".task-row-footer")?.textContent).toContain(
      "Renamed owner",
    );
    expect(renders.get("background-task-title:command") || 0).toBe(0);
    expect(renders.get("background-task-output:command") || 0).toBe(0);
    renders.clear();
    await act(async () => vi.advanceTimersByTimeAsync(1000));
    expect(renders.get("background-task-elapsed:command")).toBeGreaterThan(0);
    for (const [key, count] of renders)
      if (!key.startsWith("background-task-elapsed:"))
        expect(count, key).toBe(0);
    renders.clear();
    const changed = {
      ...process(),
      tail: "New output",
      display: { ...process().display, label: "stopping" as const },
    };
    await f.publish([changed, process("other")]);
    expect(document.querySelector(".task-status")?.textContent).toBe(
      "Stopping",
    );
    expect(renders.get("background-task-title:command") || 0).toBe(0);
    expect(renders.get("background-task-output:command") || 0).toBe(0);
    expect(renders.get("background-task-row:command") || 0).toBe(0);
    expect(renders.get("background-task-list:") || 0).toBe(0);
    expect(renders.get("background-task-row:other") || 0).toBe(0);
  } finally {
    await f.close();
  }
});
it("uses the server response target before any owner arrives and captures its proof", async () => {
  const f = await fixture([]);
  const target = {
    agent: "lead",
    accountKey: "new",
    epoch: 2,
    connectionId: "connection",
    threadId: "thread",
    turnId: "turn",
  };
  try {
    await f.publish([], target);
    await act(async () => button("Stop response").click());
    expect(h.post).toHaveBeenCalledWith(
      "/api/processes/action",
      expect.objectContaining({
        action: "stop_response",
        accountKey: "new",
        epoch: 2,
        responseTarget: target,
      }),
      expect.anything(),
    );
  } finally {
    await f.close();
  }
});
it("keeps resize and receipt controls after stdin closes and opens no resource GET", async () => {
  const task = {
    ...process(),
    source: "monitor" as const,
    display: backgroundDisplay({
      kind: "monitor",
      kindLabel: "monitor",
      stdinState: "closed",
      actions: ["resize", "readOutput"],
    }),
  };
  const f = await fixture([task]);
  try {
    expect(document.querySelector(".process-resize")).not.toBeNull();
    expect(button("Resize").disabled).toBe(false);
    expect(button("Send line").disabled).toBe(true);
    expect(button("Check input receipt").disabled).toBe(false);
    expect(h.get).not.toHaveBeenCalled();
  } finally {
    await f.close();
  }
});

it("keeps the selected activity when new membership changes its order", async () => {
  const f = await fixture([process(), process("other")]);
  try {
    await f.publish([process("new"), process(), process("other")]);
    expect(
      document
        .querySelector("[data-task-detail]")
        ?.getAttribute("data-task-detail"),
    ).toBe("command");
  } finally {
    await f.close();
  }
});

it("shows process stream failure without a fallback GET", async () => {
  const f = await fixture();
  try {
    await act(async () => {
      f.subscriptions.fail(
        { kind: "processes", agentId: "lead" },
        new Error("Process stream unavailable"),
      );
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(document.body.textContent).toContain(
      "Process list unavailable: Process stream unavailable",
    );
    expect(h.get).not.toHaveBeenCalled();
  } finally {
    await f.close();
  }
});
it("keeps one selected activity as a legacy task becomes a process", async () => {
  const f = await fixture([]);
  const task = {
    id: "command",
    agent: "lead",
    created: 2,
    status: "running" as const,
    kind: "command" as const,
    name: "command",
    turnId: "turn",
    tail: "Task output",
    display: backgroundDisplay(),
  };
  try {
    await act(async () => {
      f.subscriptions.publish(
        { kind: "processes", agentId: "lead" },
        { processes: [], activities: [{ source: "task", value: task }] },
      );
      await vi.advanceTimersByTimeAsync(0);
    });
    await act(async () =>
      document
        .querySelector<HTMLButtonElement>('[data-task="command"]')!
        .click(),
    );
    await f.publish([process()]);
    expect(document.querySelectorAll('[data-task="command"]')).toHaveLength(1);
    expect(
      document
        .querySelector("[data-task-detail]")
        ?.getAttribute("data-task-detail"),
    ).toBe("command");
    expect(document.querySelector(".task-output")?.textContent).toBe(
      "Streamed command output",
    );
  } finally {
    await f.close();
  }
});

it("keeps the open drawer mounted when its activity scope changes", async () => {
  const f = await fixture();
  try {
    const drawer = document.querySelector('[role="dialog"]');
    expect(drawer).not.toBeNull();
    await f.render("other");
    expect(document.querySelector('[role="dialog"]') === drawer).toBe(true);
  } finally {
    await f.close();
  }
});

it("opens tool input on demand through its generated scoped resource", async () => {
  const f = await fixture();
  try {
    expect(f.added.mock.calls.some(([ref]) => ref.kind === "taskDetail")).toBe(
      false,
    );
    await act(async () => button("Read tool input").click());
    await act(async () => vi.advanceTimersByTimeAsync(100));
    expect(
      f.added.mock.calls.some(
        ([ref]) => ref.kind === "taskDetail" && ref.taskId === "command",
      ),
    ).toBe(true);
    expect(h.get).not.toHaveBeenCalled();
    await act(async () => {
      f.subscriptions.publish(
        { kind: "taskDetail", taskId: "command" },
        {
          id: "command",
          agent: "lead",
          created: 2,
          status: "running",
          kind: "command",
          name: "command",
          turnId: "turn",
          arguments: "private tool arguments",
          tail: "Detailed output",
        },
      );
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(document.querySelector(".task-input pre")?.textContent).toBe(
      "private tool arguments",
    );
  } finally {
    await f.close();
  }
});

it("updates monitor wake budgets from its process subscription alone", async () => {
  const monitor = () => ({
    ...process(),
    display: backgroundDisplay({ kind: "monitor", title: "monitor" }),
    outputWakeBudget: { paused: false, totalSuppressedMatches: 2, tokens: 1 },
  });
  const f = await fixture([monitor()]);
  try {
    expect(document.querySelector(".task-wake-budget")?.textContent).toContain(
      "2 matches suppressed",
    );
    await f.publish([
      {
        ...monitor(),
        outputWakeBudget: {
          paused: true,
          totalSuppressedMatches: 7,
          tokens: 0,
        },
      },
    ]);
    expect(document.querySelector(".task-wake-budget")?.textContent).toContain(
      "7 matches suppressed",
    );
    expect(document.querySelector(".task-wake-budget")?.textContent).toContain(
      "Output wakes paused",
    );
    expect(f.added.mock.calls.some(([ref]) => ref.kind === "monitors")).toBe(
      false,
    );
  } finally {
    await f.close();
  }
});
