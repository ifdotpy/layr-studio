// @vitest-environment happy-dom
import { autorun, runInAction } from "mobx";
import { expect, it, vi } from "vitest";
import { ownerFixture } from "./ownerFixture.ts";
import { createServerStore } from "../../store/server.ts";
import { resourceData } from "../../store/runtime.ts";
import { agentId } from "../../generated/observableResources.ts";
import { serializeStateCollections } from "../../generated/observableResources.ts";
import { projectedLimits } from "../../usage/accountLimits.fixture.ts";
import {
  AccountStatus,
  ProviderKind,
  Overage,
  EligibilityState,
} from "../../generated/api.ts";
import type { components } from "../../generated/api.ts";
import {
  TaskStatus,
  TaskKind,
  BackgroundTaskKind,
  BackgroundTaskKindLabel,
  BackgroundTaskIndicator,
  BackgroundTaskLabel,
} from "../../generated/api.ts";
import { MessageSendStatus } from "../../generated/api.ts";

it("observed selected leaves receive live detail and processes with one subscription each", async () => {
  vi.useFakeTimers();
  const f = ownerFixture();
  const store = createServerStore(f.services.resources);
  vi.spyOn(f.models.store, "agentDetail").mockImplementation(store.agentDetail);
  vi.spyOn(f.models.store, "processes").mockImplementation(store.processes);
  const subscribe = vi.spyOn(f.services.resources, "subscribe");
  const stopOwner = f.selected.start();
  expect(
    subscribe.mock.calls.filter(([ref]) =>
      ["agentDetail", "processes"].includes(ref.kind),
    ),
  ).toHaveLength(0);
  const seen: unknown[] = [];
  const stopLeaf = autorun(() =>
    seen.push([f.selected.detail?.canSend, f.selected.backgroundTaskCount]),
  );
  // Another leaf shares the same generated key.
  const stopOtherLeaf = autorun(
    () => resourceData(store.agentDetail(agentId("lead")).current)?.canSend,
  );
  try {
    await vi.advanceTimersByTimeAsync(100);
    expect(
      subscribe.mock.calls
        .filter(([ref]) => ["agentDetail", "processes"].includes(ref.kind))
        .map(([ref]) => ref.kind)
        .sort(),
    ).toEqual(["agentDetail", "processes"]);
    f.services.resources.publish(
      { kind: "agentDetail", agentId: "lead" },
      { id: "lead", canSend: true },
    );
    f.services.resources.publish(
      { kind: "processes", agentId: "lead" },
      { processes: [], activities: [] },
    );
    await vi.advanceTimersByTimeAsync(0);
    expect(seen.at(-1)).toEqual([true, 0]);
    f.services.resources.publish(
      { kind: "agentDetail", agentId: "lead" },
      { id: "lead", canSend: false },
    );
    await vi.advanceTimersByTimeAsync(0);
    expect(seen.at(-1)).toEqual([false, 0]);
    const activity = {
      source: "task",
      value: {
        id: "background",
        kind: TaskKind.tool,
        name: "Background task",
        turnId: "turn",
        agent: "lead",
        created: 1,
        status: TaskStatus.running,
        display: {
          kind: BackgroundTaskKind.command,
          kindLabel: BackgroundTaskKindLabel.command,
          title: "Command",
          active: true,
          indicator: BackgroundTaskIndicator.working,
          label: BackgroundTaskLabel.running,
          inBackgroundList: true,
          actions: [],
        },
      },
    } satisfies components["schemas"]["BackgroundTaskView"];
    f.services.resources.publish(
      { kind: "processes", agentId: "lead" },
      { processes: [], activities: [activity] },
    );
    await vi.advanceTimersByTimeAsync(0);
    expect(seen.at(-1)).toEqual([false, 1]);
    runInAction(() => f.selected.suspendReaders(new Set(["lead"])));
    expect(f.selected.detail).toBeUndefined();
    f.selected.resumeReaders(new Set(["lead"]));
    expect(f.selected.detail?.canSend).toBe(false);
    expect(
      subscribe.mock.calls.filter(([ref]) =>
        ["agentDetail", "processes"].includes(ref.kind),
      ),
    ).toHaveLength(2);
  } finally {
    stopLeaf();
    stopOtherLeaf();
    stopOwner();
    store.dispose();
    f.dispose();
    vi.restoreAllMocks();
    vi.useRealTimers();
  }
});
it("selection alone does not mark a result read and disposal ignores a late CAS receipt", async () => {
  const f = ownerFixture();
  const agent = f.models.mirror.current.agents.get("lead");
  if (!agent) throw new Error("Missing lead");
  f.models.apply([
    {
      collection: "agentSummary",
      id: "lead",
      value: {
        ...agent,
        markRead: {
          threadId: "thread",
          turnId: "turn",
          read: false,
          revision: 0,
        },
      },
    },
  ]);
  let finish: ((value: unknown) => void) | undefined;
  f.post.mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  const stop = f.read.start();
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  try {
    f.read.observeRead({ id: "lead", threadId: "thread", turnId: "turn" });
    expect(f.post).not.toHaveBeenCalled();
    f.read.select("lead");
    expect(f.post).toHaveBeenCalledOnce();
    stop();
    finish?.({
      id: "lead",
      readState: {
        threadId: "thread",
        turnId: "turn",
        read: true,
        revision: 1,
      },
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(f.read.scope.states.size).toBe(0);
    expect(f.notify).not.toHaveBeenCalled();
  } finally {
    stop();
    f.dispose();
    vi.restoreAllMocks();
  }
});

it("deleting another project leaves the selected conversation readers admitted", () => {
  const f = ownerFixture();
  const release = vi.fn();
  const subscribe = vi
    .spyOn(f.services.resources, "subscribe")
    .mockReturnValue(release);
  const stop = f.selected.start();
  try {
    f.selected.suspendReaders(new Set(["different-project-agent"]));
    expect(release).not.toHaveBeenCalled();
    expect(subscribe).not.toHaveBeenCalled();
    expect(f.selected.detail?.id).toBe("lead");
  } finally {
    stop();
    f.dispose();
    vi.restoreAllMocks();
  }
});

it("a pending standalone deletion suspends the selected conversation", () => {
  const f = ownerFixture();
  try {
    f.selected.pendingDeletion.set("lead", new AbortController());
    expect(f.selected.isSelectionSuspended).toBe(true);
  } finally {
    f.dispose();
  }
});

it("receives a receipt published after the selected owner starts", () => {
  const f = ownerFixture();
  const stop = f.selected.start();
  try {
    runInAction(() => {
      f.selected.history.items = [
        {
          id: "lead:send",
          clientMessageId: "send",
          role: "user",
          text: "pending",
          deliveryStatus: { source: "device", state: { kind: "stored" } },
        },
      ];
    });
    expect(f.selected.receipts.has("send")).toBe(false);
    f.models.apply([
      ...serializeStateCollections(f.models.mirror.current),
      {
        collection: "messageReceipt",
        id: "send",
        value: {
          id: "send",
          agent: "lead",
          status: MessageSendStatus.delivered,
        },
      },
    ]);
    expect(f.selected.receipts.get("send")?.status).toBe(
      MessageSendStatus.delivered,
    );
  } finally {
    stop();
    f.dispose();
  }
});

it("plan and limit recovery follow the selected agent account A to B to A", () => {
  const f = ownerFixture();
  const accounts = ["A", "B"].map((id) => ({
    id,
    accountId: "native-" + id,
    label: id,
    provider: ProviderKind.codex,
    status: AccountStatus.ready,
    home: "",
    source: "fixture",
  }));
  const limits = accounts.map((account, index) => {
    const view = projectedLimits(account.id, 100);
    view.observation.data = {
      accountId: account.accountId,
      rateLimits: { planType: index ? "pro" : "plus" },
    };
    view.projection.agents.lead = {
      eligibility: {
        accountKey: account.id,
        agentId: "lead",
        buckets: [],
        model: "fixture",
        overage: Overage.unavailable,
        provider: ProviderKind.codex,
        state: EligibilityState.blocked,
        validUntil: null,
      },
      recovery: {
        title: "Recovery " + account.id,
        message: account.id,
        timing: { kind: "unknown" },
        action: { kind: "none" },
      },
    };
    return { collection: "rateLimit" as const, id: account.id, value: view };
  });
  f.models.apply([
    ...serializeStateCollections(f.models.mirror.current),
    {
      collection: "accounts",
      id: "current",
      value: {
        accounts,
        archivedAccounts: [],
        defaultAccountKey: "A",
        defaultAccountGroups: [],
        supportsDelete: true,
        supportsDisconnect: true,
      },
    },
    ...limits,
  ]);
  const select = (accountKey: string) =>
    f.models.apply(
      serializeStateCollections(f.models.mirror.current).map((row) =>
        row.collection === "agentSummary" && row.id === "lead"
          ? { ...row, value: { ...row.value, accountKey } }
          : row,
      ),
    );
  select("A");
  const seen: unknown[] = [];
  const stop = autorun(() =>
    seen.push([f.selected.planType, f.selected.limitRecovery?.title]),
  );
  try {
    select("B");
    select("A");
    expect(seen).toEqual([
      ["plus", "Recovery A"],
      ["pro", "Recovery B"],
      ["plus", "Recovery A"],
    ]);
    expect(f.post).not.toHaveBeenCalled();
  } finally {
    stop();
    f.dispose();
  }
});

it.each(["before-final-failure", "after-final-failure"] as const)(
  "streamed read confirmation %s settles uncertainty without another write",
  async (timing) => {
    vi.useFakeTimers();
    const f = ownerFixture();
    const proof = { id: "lead", threadId: "thread", turnId: "turn" };
    const publish = (read: boolean, revision: number) => {
      const agent = f.models.mirror.current.agents.get("lead")!;
      f.models.apply([
        ...serializeStateCollections(f.models.mirror.current).filter(
          (row) => row.collection !== "agentSummary" || row.id !== "lead",
        ),
        {
          collection: "agentSummary",
          id: "lead",
          value: {
            ...agent,
            markRead: { ...proof, read, revision },
            readState: {
              threadId: proof.threadId,
              turnId: proof.turnId,
              read,
              revision,
            },
          },
        },
      ]);
    };
    publish(false, 0);
    const stop = f.read.start();
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    const replace = vi.spyOn(f.services.resources, "replaceStream");
    f.post.mockImplementation(async () => {
      if (timing === "before-final-failure" && f.post.mock.calls.length === 3)
        publish(true, 1);
      throw new TypeError("Read response lost");
    });
    try {
      const write = f.read.write(proof, true);
      await vi.advanceTimersByTimeAsync(3100);
      await write;
      if (timing === "after-final-failure") publish(true, 1);
      expect(f.read.scope.uncertain.has("lead")).toBe(false);
      if (timing === "before-final-failure")
        expect(f.notify).not.toHaveBeenCalled();
      f.notify.mockClear();
      const sent = f.post.mock.calls.length;
      f.read.select("lead");
      await vi.advanceTimersByTimeAsync(3100);
      expect(f.post).toHaveBeenCalledTimes(sent);
      expect(f.notify).not.toHaveBeenCalled();
      expect(f.services.sync.refreshState).not.toHaveBeenCalled();
      expect(replace).not.toHaveBeenCalled();
    } finally {
      stop();
      f.dispose();
      vi.restoreAllMocks();
      vi.useRealTimers();
    }
  },
);

it("accepts an idempotent read receipt with the existing confirmed revision", async () => {
  const f = ownerFixture();
  const proof = { id: "lead", threadId: "thread", turnId: "turn" };
  const agent = f.models.mirror.current.agents.get("lead")!;
  const readState = {
    threadId: "thread",
    turnId: "turn",
    read: false,
    revision: 1,
  };
  f.models.apply([
    {
      collection: "agentSummary",
      id: "lead",
      value: {
        ...agent,
        markRead: readState,
        readState,
      },
    },
  ]);
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  f.post.mockResolvedValue({
    id: "lead",
    readState: { ...readState, read: true },
  });
  try {
    await f.read.write(proof, true);
    expect(f.read.scope.states.get("lead")).toMatchObject({
      read: true,
      revision: 1,
    });
    expect(f.notify).not.toHaveBeenCalled();
  } finally {
    f.dispose();
    vi.restoreAllMocks();
  }
});

it("a superseding Unread stream settles a lost Read and confirms an idempotent lost Unread", async () => {
  vi.useFakeTimers();
  const f = ownerFixture();
  const proof = { id: "lead", threadId: "thread", turnId: "turn" };
  const publish = (read: boolean, revision: number) => {
    const agent = f.models.mirror.current.agents.get("lead")!;
    const state = {
      threadId: proof.threadId,
      turnId: proof.turnId,
      read,
      revision,
    };
    f.models.apply([
      {
        collection: "agentSummary",
        id: "lead",
        value: {
          ...agent,
          markRead: state,
          readState: state,
        },
      },
    ]);
  };
  publish(false, 0);
  const stop = f.read.start();
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  const replace = vi.spyOn(f.services.resources, "replaceStream");
  f.post.mockRejectedValue(new TypeError("Read response lost"));
  try {
    const read = f.read.write(proof, true);
    await vi.advanceTimersByTimeAsync(3100);
    await read;
    expect(f.read.scope.uncertain.has("lead")).toBe(true);
    // Another client committed Unread after our Read, before its SSE arrived.
    publish(false, 2);
    expect(f.read.scope.uncertain.has("lead")).toBe(false);
    // Explicit idempotent Unread has the same canonical revision and loses its receipt.
    f.notify.mockClear();
    await f.read.write(proof, false, {
      threadId: "thread",
      turnId: "turn",
      read: true,
      revision: 2,
    });
    publish(false, 2);
    expect(f.read.scope.uncertain.has("lead")).toBe(false);
    expect(f.read.scope.states.get("lead")).toMatchObject({
      read: false,
      revision: 2,
    });
    expect(f.notify).not.toHaveBeenCalled();
    expect(f.services.sync.refreshState).not.toHaveBeenCalled();
    expect(replace).not.toHaveBeenCalled();
  } finally {
    stop();
    f.dispose();
    vi.restoreAllMocks();
    vi.useRealTimers();
  }
});
