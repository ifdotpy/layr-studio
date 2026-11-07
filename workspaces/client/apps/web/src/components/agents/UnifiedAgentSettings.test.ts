vi.mock("mobx-react-lite", async (original) => ({
  ...(await original<typeof import("mobx-react-lite")>()),
  observer: <T>(component: T) => component,
}));
vi.mock("./state.ts", () => ({ useAgentDetail: () => undefined }));
import { beforeEach, expect, test, vi } from "vitest";
import type { ComponentProps } from "react";
import { UnifiedAgentSettingsView as UnifiedAgentSettings } from "./UnifiedAgentSettings";

const harness = vi.hoisted(() => ({
  states: [false, false],
  index: 0,
  post: vi.fn(),
}));
vi.mock("react", async (load) => ({
  ...(await load<typeof import("react")>()),
  useState: () => {
    const index = harness.index++;
    return [
      harness.states[index],
      (value: boolean) => {
        harness.states[index] = value;
      },
    ];
  },
  useCallback: (callback: unknown) => callback,
}));
vi.mock("../../servers/ServerServicesContext", () => ({
  useServerApi: () => ({ post: harness.post }),
}));

beforeEach(() => {
  harness.states = [false, false];
  harness.index = 0;
  harness.post.mockReset();
});

test("keeps exclusions locked through refresh so the next change retains the first", async () => {
  let completeRefresh!: () => void;
  const refresh = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        completeRefresh = resolve;
      }),
  );
  harness.post.mockResolvedValue({ agent: { id: "lead" } });
  let excluded: string[] = [];
  const render = () => {
    harness.index = 0;
    const component = UnifiedAgentSettings({
      state: {
        data: {
          accounts: [],
          archivedAccounts: [],
          defaultAccountGroups: [],
          defaultAccountKey: "default",
          logins: [],
          supportsDelete: false,
          supportsDisconnect: false,
        },
        setData: vi.fn(),
        error: "",
        refresh: vi.fn(),
        scope: "",
      },
      agent: {
        id: "lead",
        isLead: true,
        workerExcludedAccountKeys: excluded,
        reviewerExcludedAccountKeys: [],
      } as ComponentProps<typeof UnifiedAgentSettings>["agent"],
      catalog: { models: [], loading: false, error: "", retry: () => {} },
      refresh,
      notify: vi.fn(),
    });
    const picker = component.props.renderPicker(() => {}, false);
    return picker.props.children.props;
  };
  render().onWorkerExcludedAccountKeysChange(["A"]);
  await Promise.resolve();
  expect(refresh).toHaveBeenCalledOnce();
  expect(refresh).toHaveBeenCalledWith();
  expect(render().accountDisabled).toBe(true);
  excluded = ["A"];
  completeRefresh();
  await vi.waitFor(() => expect(render().accountDisabled).toBe(false));
  render().onWorkerExcludedAccountKeysChange([...excluded, "B"]);
  expect(harness.post).toHaveBeenLastCalledWith("/api/conversation", {
    id: "lead",
    worker_excluded_account_keys: ["A", "B"],
  });
  render().onReviewerExcludedAccountKeysChange(["B"]);
  expect(harness.post).toHaveBeenLastCalledWith("/api/conversation", {
    id: "lead",
    reviewer_excluded_account_keys: ["B"],
  });
});
