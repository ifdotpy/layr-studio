import { AgentsUi } from "../support/AgentsUi.ts";
import { DEFAULT_SUBAGENT_CONCURRENCY } from "../../../generated/apiSchema.ts";
import { flowResult, action } from "mobx";
import { afterEach, expect, test, vi } from "vitest";
import { ApiError } from "../../../api.ts";
import { createApiStorage } from "../../../apiStorage.ts";
import {
  AgentMode,
  AgentStatus,
  ProviderKind,
} from "../../../generated/api.ts";
import type { Agent } from "../../../types.ts";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../shell/integration-support/observableFixture.tsx";
import { ConcurrencyStore } from "../SubagentConcurrencyControl/ConcurrencyStore.ts";

const fixtures: ReturnType<typeof observableFixture>[] = [];
function fixture() {
  const agent: Agent = {
    id: "lead",
    accountKey: "default",
    provider: ProviderKind["claude"],
    status: AgentStatus["idle"],
    concurrency: 32,
    agentModeRevision: 0,
    subagentConcurrencyVersion: 2,
  };
  const owner = observableFixture([agent]);
  fixtures.push(owner);
  const values = new Map<string, string>();
  const storage: Storage = {
    get length() {
      return values.size;
    },
    key: (i) => [...values.keys()][i] ?? null,
    getItem: (k) => values.get(k) ?? null,
    setItem: (k, v) => {
      values.set(k, v);
    },
    removeItem: (k) => {
      values.delete(k);
    },
    clear: () => values.clear(),
  };
  const api = { ...createApiStorage(storage), post: vi.fn() };
  return { agent, owner, storage, api, values };
}
afterEach(() => {
  for (const fixture of fixtures.splice(0)) fixture.store.dispose();
});

test("concurrency locks duplicate clicks, retains a lost request across remount, and replays its exact body", async () => {
  const f = fixture();
  const pending = Promise.withResolvers<Agent>();
  f.api.post.mockReturnValue(pending.promise);
  const control = new ConcurrencyStore(
    f.agent,
    f.owner.store,
    f.api,
    f.storage,
    "scope",
    "workspace",
  );
  control.setDraft(64);
  const first = flowResult(control.submit());
  expect(control.saving).toBe(true);
  expect(control.disabled).toBe(true);
  await flowResult(control.submit());
  expect(f.api.post).toHaveBeenCalledOnce();
  const request = f.api.post.mock.calls[0]?.[1];
  expect([...f.values.keys()][0]).toBe("studio-agent-mode:scope");
  pending.reject(new Error("response lost"));
  await first;
  expect(control.saving).toBe(false);
  expect(control.error).toBe("response lost");
  const reopened = new ConcurrencyStore(
    f.agent,
    f.owner.store,
    f.api,
    f.storage,
    "scope",
    "workspace",
  );
  expect(reopened.draft).toBe(64);
  f.api.post.mockResolvedValue({
    ...f.agent,
    concurrency: 64,
    agentModeRevision: 1,
  });
  await flowResult(reopened.submit(128));
  expect(f.api.post.mock.calls[1]?.[1]).toEqual(request);
  expect(reopened.pending).toBeUndefined();
  expect(f.values.size).toBe(0);
  expect(reopened.draft).toBe(32);
  f.owner.details
    .get(f.agent.id)
    ?.apply({ ...f.agent, concurrency: 64, agentModeRevision: 1 });
  expect(reopened.draft).toBe(64);
});

test("concurrency preserves dirty edits and reads only resource values after a command", async () => {
  const f = fixture();
  const control = new ConcurrencyStore(
    f.agent,
    f.owner.store,
    f.api,
    f.storage,
    "scope",
    "workspace",
  );
  control.setDraft(64);
  f.owner.details
    .get(f.agent.id)
    ?.apply({ ...f.agent, concurrency: 48, agentModeRevision: 1 });
  expect(control.draft).toBe(64);
  expect(control.confirmed?.concurrency).toBe(48);
  f.api.post.mockResolvedValue({
    ...f.agent,
    concurrency: 64,
    agentModeRevision: 3,
  });
  await flowResult(control.submit());
  expect(f.api.post.mock.calls[0]?.[1]).toMatchObject({
    expected_mode_revision: 1,
  });
  expect(control.draft).toBe(48);
  f.owner.details
    .get(f.agent.id)
    ?.apply({ ...f.agent, concurrency: 96, agentModeRevision: 2 });
  expect(control.draft).toBe(96);
  f.owner.details
    .get(f.agent.id)
    ?.apply({ ...f.agent, concurrency: 128, agentModeRevision: 4 });
  expect(control.draft).toBe(128);
});

test("concurrency clears a definitely rejected receipt and never posts when persistence fails", async () => {
  const f = fixture();
  const control = new ConcurrencyStore(
    f.agent,
    f.owner.store,
    f.api,
    f.storage,
    "scope",
    "workspace",
  );
  f.api.post.mockRejectedValue(
    new ApiError("conflict", 409, { outcome: "not_applied" }),
  );
  control.setDraft(64);
  await flowResult(control.submit());
  expect(control.pending).toBeUndefined();
  expect(control.error).toBe("conflict");
  expect(control.draft).toBe(32);
  const write = vi.spyOn(f.storage, "setItem").mockImplementation(() => {
    throw new Error("full");
  });
  control.setDraft(64);
  await flowResult(control.submit());
  expect(f.api.post).toHaveBeenCalledOnce();
  expect(control.error).toContain("Cannot save");
  write.mockRestore();
});

test("a late concurrency acknowledgment preserves a newer persisted request", async () => {
  const f = fixture(),
    pending = Promise.withResolvers<Agent>();
  f.api.post.mockReturnValue(pending.promise);
  const control = new ConcurrencyStore(
    f.agent,
    f.owner.store,
    f.api,
    f.storage,
    "scope",
    "workspace",
  );
  control.setDraft(64);
  const save = flowResult(control.submit());
  const newer = {
    id: f.agent.id,
    request_id: "newer",
    subagent_concurrency: 128,
    expected_mode_revision: 1,
    expected_account_key: "default",
  };
  const key = [...f.values.keys()][0];
  if (!key) throw new Error("Expected saved request");
  f.storage.setItem(key, JSON.stringify({ pending: newer }));
  pending.resolve({ ...f.agent, concurrency: 64, agentModeRevision: 1 });
  await save;
  expect(control.pending).toEqual(newer);
  expect(control.draft).toBe(128);
  expect(JSON.parse(f.storage.getItem(key) || "null")).toEqual({
    pending: newer,
  });
});

test("browser receipt fixture addresses the existing scoped receipt key", async () => {
  const { installConcurrencyStorageFixture } =
    await import("../../../../tests/teams/subagent-concurrency.fixture.ts");
  vi.stubGlobal("window", {});
  const f = fixture(),
    pending = Promise.withResolvers<Agent>();
  f.api.post.mockReturnValue(pending.promise);
  const stateDir = "state",
    workspaceId = "workspace";
  installConcurrencyStorageFixture({
    stateDir,
    workspaceId,
  });
  const scope = JSON.stringify([stateDir, workspaceId, f.agent.id]);
  const control = new ConcurrencyStore(
    f.agent,
    f.owner.store,
    f.api,
    f.storage,
    scope,
    workspaceId,
  );
  control.setDraft(64);
  const save = flowResult(control.submit());
  try {
    expect(window.concurrencyStorageFixture(f.agent.id)).toBe(
      [...f.values.keys()][0],
    );
  } finally {
    pending.resolve({ ...f.agent, concurrency: 64, agentModeRevision: 1 });
    await save;
    vi.unstubAllGlobals();
  }
});

test("an uncertain request saved under the existing key replays the same identity", async () => {
  const f = fixture(),
    scope = JSON.stringify(["state", "workspace", f.agent.id]);
  const request = {
    id: f.agent.id,
    subagent_concurrency: 64,
    expected_mode_revision: 0,
    request_id: "saved-uncertain-request",
    expected_account_key: "default",
  };
  f.storage.setItem(
    `studio-agent-mode:${scope}`,
    JSON.stringify({ pending: request }),
  );
  const control = new ConcurrencyStore(
    f.agent,
    f.owner.store,
    f.api,
    f.storage,
    scope,
    "workspace",
  );
  expect(control.pending).toEqual(request);
  expect(control.draft).toBe(64);
  f.api.post.mockResolvedValue({
    ...f.agent,
    concurrency: 64,
    agentModeRevision: 1,
  });
  await flowResult(control.submit(128));
  expect(f.api.post).toHaveBeenCalledExactlyOnceWith(
    "/api/conversation",
    request,
    { workspaceId: "workspace", timeoutMs: 15000 },
  );
  expect(control.pending).toBeUndefined();
  expect(control.confirmed?.concurrency).toBe(32);
});

test.each([AgentMode["single"], AgentMode["multi"]])(
  "an uncertain %s mode receipt stays locked and replays its original body after remount",
  async (mode) => {
    const f = fixture(),
      scope = JSON.stringify(["state", "workspace", f.agent.id]);
    const request = {
      id: f.agent.id,
      agent_mode: mode,
      expected_mode_revision: 0,
      request_id: `saved-mode-${mode}`,
      expected_account_key: "default",
    };
    const key = `studio-agent-mode:${scope}`;
    f.storage.setItem(key, JSON.stringify({ pending: request }));
    const control = new ConcurrencyStore(
      f.agent,
      f.owner.store,
      f.api,
      f.storage,
      scope,
      "workspace",
    );
    expect(control.pending).toEqual(request);
    expect(control.disabled).toBe(true);
    expect(control.draft).toBe(
      mode === AgentMode["single"] ? 0 : DEFAULT_SUBAGENT_CONCURRENCY,
    );
    f.api.post.mockRejectedValueOnce(new Error("response lost"));
    await flowResult(control.submit(128));
    expect(f.api.post.mock.calls[0]?.[1]).toEqual(request);
    expect(JSON.parse(f.storage.getItem(key) || "null")).toEqual({
      pending: request,
    });
    const reopened = new ConcurrencyStore(
      f.agent,
      f.owner.store,
      f.api,
      f.storage,
      scope,
      "workspace",
    );
    expect(reopened.disabled).toBe(true);
    f.api.post.mockResolvedValue(f.agent);
    await flowResult(reopened.submit(2048));
    expect(f.api.post.mock.calls[1]?.[1]).toEqual(request);
    expect(reopened.pending).toBeUndefined();
    expect(f.storage.getItem(key)).toBeNull();
  },
);

// Selection lifetime is owned by ServerAreas; no mounted component is needed.
test("the server owner invalidates an in-flight settings receipt on selection change", () => {
  const f = fixture();
  const areas = conversationActionsFixture(undefined, f.owner.store);
  try {
    action(() => {
      areas.navigation.selectedId = f.agent.id;
    })();
    const accept = areas.agentsUi.begin(
      "settings",
      "orchestrator",
      () => "unchanged",
    );
    action(() => {
      areas.navigation.selectedId = "other-chat";
    })();
    expect(accept({ ...f.agent, model: "gpt-6-luna" })).toBe(false);
    areas.agentsUi.set("settings", "error", "pending");
    areas.dispose();
    expect(areas.agentsUi.fields.size).toBe(0);
    expect(areas.agentsUi.concurrency.size).toBe(0);
  } finally {
    areas.dispose();
  }
});

test("an older same-domain owner receipt cannot replace the latest acknowledgment", () => {
  const owner = new AgentsUi();
  try {
    const first = owner.begin(
      "settings",
      "orchestrator",
      () => "same projection",
    );
    const latest = owner.begin(
      "settings",
      "orchestrator",
      () => "same projection",
    );
    expect(latest({ id: "lead", model: "latest" })).toBe(true);
    expect(first({ id: "lead", model: "older" })).toBe(false);
    expect(
      owner.fields.get("settings")?.confirmed.orchestrator?.value.model,
    ).toBe("latest");
  } finally {
    owner.dispose();
  }
});
