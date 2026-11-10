// @vitest-environment happy-dom
vi.mock("mobx-react-lite", async (original) => ({
  ...(await original<typeof import("mobx-react-lite")>()),
  observer: <T>(component: T) => component,
}));
import { beforeEach, expect, test, vi } from "vitest";
import { act, createElement, type ComponentProps } from "react";
import { createRoot } from "react-dom/client";
import { ApiError } from "../../../api.ts";
import { createMutationJournal } from "../../../servers/mutationIdentity.ts";
import { UnifiedAgentSettingsView as UnifiedAgentSettings } from "./UnifiedAgentSettings.tsx";

const harness = vi.hoisted(() => ({
  post: vi.fn(),
  excluded: [] as string[],
  version: { epoch: "session", revision: 1 },
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerStore: () => ({
      agentDetail: () => ({
        appliedVersion: harness.version,
        current: {
          status: "live",
          data: {
            workerExcludedAccountKeys: harness.excluded,
            reviewerExcludedAccountKeys: [],
          },
        },
      }),
    }),
    useServerApi: () => ({ post: harness.post }),
  }),
);

beforeEach(() => {
  harness.post.mockReset();
  harness.excluded = [];
  harness.version = { epoch: "session", revision: 1 };
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
});

function mount() {
  const refresh = vi.fn();
  const notify = vi.fn();
  let current!: ReturnType<typeof UnifiedAgentSettings>;
  const root = createRoot(document.createElement("div"));
  function Fixture() {
    current = UnifiedAgentSettings({
      state: {
        accounts: [],
        archivedAccounts: [],
        defaultAccountGroups: [],
        defaultAccountKey: "default",
        supportsDelete: false,
        supportsDisconnect: false,
      },
      agent: { id: "lead", isLead: true } as ComponentProps<
        typeof UnifiedAgentSettings
      >["agent"],
      catalog: { models: [], loading: false, error: "", retry: () => {} },
      notify,
    });
    return null;
  }
  const render = () => act(() => root.render(createElement(Fixture)));
  render();
  return {
    picker: () =>
      current.props.renderPicker(() => {}, false).props.children.props,
    publish(keys: string[], revision: number, epoch = "session") {
      harness.excluded = keys;
      harness.version = { epoch, revision };
      render();
    },
    refresh,
    notify,
    close: () => act(() => root.unmount()),
  };
}

test("keeps a lost response locked through stale SSE and bases the next edit on the next publication", async () => {
  const rows = new Map<string, string>();
  const storage: Storage = {
    get length() {
      return rows.size;
    },
    clear: () => rows.clear(),
    key: (index) => [...rows.keys()][index] ?? null,
    getItem: (key) => rows.get(key) ?? null,
    setItem: (key, value) => {
      rows.set(key, value);
    },
    removeItem: (key) => {
      rows.delete(key);
    },
  };
  const journal = createMutationJournal("exclusions", storage);
  const identities: string[] = [];
  harness.post.mockImplementation(async (path, body) => {
    const request = (await journal(path, body))!;
    identities.push(request.requestId);
    request.unknown();
    throw new TypeError("committed response lost");
  });
  const f = mount();
  try {
    await act(async () => {
      f.picker().onWorkerExcludedAccountKeysChange(["A"]);
      await Promise.allSettled(
        harness.post.mock.results.map((result) => result.value),
      );
    });
    expect(f.picker().accountDisabled).toBe(true);
    f.publish([], 1);
    expect(f.picker().accountDisabled).toBe(true);
    await act(async () => {
      f.picker().onWorkerExcludedAccountKeysChange(["A"]);
      await Promise.allSettled(
        harness.post.mock.results.map((result) => result.value),
      );
    });
    expect(identities).toHaveLength(2);
    expect(identities[1]).toBe(identities[0]);
    f.publish(["A"], 2);
    expect(f.picker().accountDisabled).toBe(false);
    expect(f.refresh).not.toHaveBeenCalled();
    harness.post.mockResolvedValue({});
    await act(async () =>
      f.picker().onWorkerExcludedAccountKeysChange([...harness.excluded, "B"]),
    );
    expect(harness.post).toHaveBeenLastCalledWith("/api/conversation", {
      id: "lead",
      worker_excluded_account_keys: ["A", "B"],
    });
  } finally {
    f.close();
  }
});

test("unlocks a successful save superseded by another client's authoritative publication", async () => {
  harness.post.mockResolvedValue({});
  const f = mount();
  try {
    await act(async () => {
      f.picker().onWorkerExcludedAccountKeysChange(["A"]);
      await Promise.allSettled(
        harness.post.mock.results.map((result) => result.value),
      );
    });
    expect(f.picker().accountDisabled).toBe(true);
    f.publish(["C"], 2);
    expect(f.picker().accountDisabled).toBe(false);
    expect(f.picker().input.detail.workerExcludedAccountKeys).toEqual(["C"]);
    await act(async () =>
      f.picker().onWorkerExcludedAccountKeysChange([...harness.excluded, "B"]),
    );
    expect(harness.post).toHaveBeenLastCalledWith("/api/conversation", {
      id: "lead",
      worker_excluded_account_keys: ["C", "B"],
    });
  } finally {
    f.close();
  }
});

test.each([400, 503])(
  "HTTP %s unlocks only when the response is a definite rejection",
  async (status) => {
    harness.post.mockRejectedValue(new ApiError("failed", status));
    const f = mount();
    try {
      await act(async () =>
        f.picker().onWorkerExcludedAccountKeysChange(["A"]),
      );
      expect(f.picker().accountDisabled).toBe(status === 503);
      expect(f.notify).toHaveBeenCalledOnce();
    } finally {
      f.close();
    }
  },
);
