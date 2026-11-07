import { autorun, toJS } from "mobx";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { TerminalState, ProviderKind } from "../generated/api.ts";
import { createServerStore } from "./server.ts";
import { createWorkDataSelector } from "../sync/subscriptions/workData.ts";
import type { ResourceRef } from "../sync/subscriptions/contracts.ts";
import { createSubscriptions } from "../sync/subscriptions/module.ts";
import {
  agentId,
  createWorkMirror,
  createTerminalMirror,
  createTerminalOutputMirror,
} from "../generated/observableResources.ts";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
it("acquires the work and deleted-worker cleanup feeds only while observed", async () => {
  const watch = vi.fn((_ref: ResourceRef) => () => {});
  const subscriptions = createSubscriptions({
    watchResourceChanges: watch,
    watchResourceConnection: (accept) => {
      accept("live", "open", {});
      return () => {};
    },
  });
  const store = createServerStore(subscriptions);
  expect(store.work).toBeTypeOf("function");
  expect(store.retainedWorktrees).toBeTypeOf("function");
  const work = store.work(agentId("lead"));
  const retained = store.retainedWorktrees(agentId("lead"));
  void work.current;
  void retained.current;
  await vi.advanceTimersByTimeAsync(100);
  expect(watch).not.toHaveBeenCalled();
  const stop = autorun(() => {
    void work.current;
    void retained.current;
  });
  await vi.advanceTimersByTimeAsync(100);
  expect(watch.mock.calls.map(([ref]) => ref)).toEqual([
    { kind: "work", agentId: "lead" },
    { kind: "retained-worktrees", agentId: "lead" },
  ]);
  stop();
  store.dispose();
  subscriptions.dispose();
});

it("preserves the existing work projection shape, evidence and gate policy", () => {
  const item = {
    id: "assignment",
    rootId: "lead",
    title: "Implement",
    status: "review" as const,
    created: 1,
    updated: 2,
    version: 1,
    policy: {
      blockedBy: [],
      displayStatus: "review" as const,
      acceptAvailable: true,
      gateOverrideRequired: false,
    },
    gate: { command: "just test-fast", timeout_ms: 600000 },
    results: [
      {
        id: "result",
        agent: "worker",
        checks: "passed",
        created: 2,
        files: [],
        revision: "state",
        text: "done",
        result: { count: 1 },
        gate: {
          status: "passed" as const,
          config: "just test-fast",
          cwd: "/fixture",
          revision: "state",
          monitorId: null,
          exitCode: 0,
          output: "passed",
          log: null,
        },
      },
    ],
    decisions: [],
    releases: [],
  };
  const existing = createWorkDataSelector()(
    [
      {
        collection: "workDetail",
        id: item.id,
        value: { ...item, results: [] },
      },
      {
        collection: "workResult",
        id: "assignment:0",
        value: {
          id: "assignment:0",
          rootId: "lead",
          taskId: item.id,
          index: 0,
          value: item.results[0]!,
        },
      },
    ],
    "lead",
  );
  const mirror = createWorkMirror(existing);
  const json = JSON.stringify(toJS(mirror.current));
  expect(JSON.parse(json)).toEqual(existing);
  const row = mirror.current.items[0];
  const gate = row?.results?.[0]?.gate;
  mirror.apply(structuredClone(existing));
  expect(mirror.current.items[0]).toBe(row);
  expect(mirror.current.items[0]?.results?.[0]?.gate).toBe(gate);
  const changed = {
    ...existing,
    items: [
      {
        ...existing.items[0]!,
        results: [
          {
            ...item.results[0]!,
            gate: { ...item.results[0]!.gate, output: "updated" },
          },
        ],
      },
    ],
  };
  mirror.apply(changed);
  expect(mirror.current.items[0]).toBe(row);
  expect(mirror.current.items[0]?.results?.[0]?.gate).toBe(gate);
  expect(gate?.output).toBe("updated");
});

it("updates terminal metadata and the existing output window in place", () => {
  const terminal = {
    id: "shell",
    agent: "lead",
    title: "Shell",
    cwd: "/fixture",
    status: TerminalState.running,
    created: 1,
  };
  const metadata = createTerminalMirror(terminal);
  const output = createTerminalOutputMirror({
    text: "before",
    offset: 6,
    truncated: false,
    status: TerminalState.running,
  });
  const model = output.current;
  metadata.apply({ ...terminal, title: "Renamed" });
  output.apply({
    text: "after",
    offset: 11,
    truncated: true,
    status: TerminalState.exited,
    exitCode: 0,
  });
  expect(metadata.current.title).toBe("Renamed");
  expect(output.current).toBe(model);
  expect(model.text).toBe("after");
  expect(model.offset).toBe(11);
});

it("delivers model catalogs through the shared state subscription", async () => {
  const watch = vi.fn((_ref: ResourceRef) => () => {});
  const subscriptions = createSubscriptions({
    watchResourceChanges: watch,
    watchResourceConnection: (accept) => {
      accept("live", "open", {});
      return () => {};
    },
  });
  const store = createServerStore(subscriptions);
  const seen: string[][] = [];
  const stop = autorun(() => {
    const current = store.models().current;
    seen.push(
      current.status === "live" || current.status === "stale"
        ? [...current.data.modelsByAccount.values()].map((row) => row.model)
        : [],
    );
    void store.state().current;
  });
  await vi.advanceTimersByTimeAsync(100);
  expect(watch.mock.calls.map(([ref]) => ref)).toEqual([{ kind: "state" }]);
  subscriptions.publish({ kind: "state" }, [
    {
      collection: "model",
      id: "default:model",
      value: {
        accountKey: "default",
        model: "gpt-6-astra",
        provider: ProviderKind.codex,
        catalogPosition: 0,
      },
    },
  ]);
  await vi.advanceTimersByTimeAsync(0);
  expect(seen.at(-1)).toEqual(["gpt-6-astra"]);
  expect(store.models().current.status).not.toBe("loading");
  stop();
  store.dispose();
  subscriptions.dispose();
});
