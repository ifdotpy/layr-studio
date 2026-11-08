import { expect, test } from "vitest";
import {
  isNewChatSettings,
  projectAllowsWorktree,
  settingsForNewChatRequest,
  settingsForNewChatAccount,
  type NewChatWorkspaceMode,
} from "./newChatSettings.ts";
import { isServerCommand } from "../../servers/navigation.ts";
import { ACCOUNT_KEY_MAX_CHARS } from "../../generated/sharedValues.ts";

test.each(["layr-native", "worktree"] satisfies NewChatWorkspaceMode[])(
  "accepts the generated new-chat workspace mode %s",
  (workspaceMode) => expect(isNewChatSettings({ workspaceMode })).toBe(true),
);

test("new chat preserves independent worker and reviewer exclusions", () => {
  const worker_excluded_account_keys = ["worker"];
  const reviewer_excluded_account_keys = ["reviewer"];
  const settings = {
    worker_excluded_account_keys,
    reviewer_excluded_account_keys,
  };
  expect(isNewChatSettings({ ...settings, workspaceMode: "worktree" })).toBe(
    true,
  );
  expect(
    settingsForNewChatRequest(settings, "lead", "gpt-6-astra"),
  ).toMatchObject(settings);
  expect(
    isNewChatSettings({
      ...settings,
      workspaceMode: "worktree",
      reviewer_excluded_account_keys: [42],
    }),
  ).toBe(false);
  expect(
    isNewChatSettings({
      ...settings,
      workspaceMode: "worktree",
      worker_excluded_account_keys: ["x".repeat(ACCOUNT_KEY_MAX_CHARS + 1)],
    }),
  ).toBe(false);
});

test.each([
  undefined,
  null,
  "",
  "shared",
  "unknown",
  { toString: () => "worktree" },
  ["layr"],
])("rejects malformed new-chat workspace mode %s", (workspaceMode) => {
  expect(isNewChatSettings({ workspaceMode })).toBe(false);
});

test("new chat accepts explicit reviewer Fast mode and rejects malformed review fields", () => {
  expect(
    isNewChatSettings({
      workspaceMode: "worktree",
      review_defaults: { model: null, effort: null, account_key: "reviewer" },
    }),
  ).toBe(false);
  for (const fast_mode of [true, false]) {
    expect(
      isNewChatSettings({
        workspaceMode: "worktree",
        review_defaults: { model: null, effort: null, fast_mode },
      }),
    ).toBe(true);
  }
  for (const patch of [
    { fast_mode: "false" },
    { fast_mode: null },
    { unknown: false },
    { account_key: 42 },
  ]) {
    expect(
      isNewChatSettings({
        workspaceMode: "worktree",
        review_defaults: { model: null, effort: null, ...patch },
      }),
    ).toBe(false);
  }
});

test("accepts the actual selector values and rejects receipt overrides", () => {
  const settings = {
    workspaceMode: "layr-native",
    model: "gpt-6-astra",
    effort: "high",
    fast_mode: true,
    worker_defaults: {
      model: null,
      effort: null,
      fast_mode: false,
    },
    review_defaults: { model: null, effort: null },
  };
  expect(isNewChatSettings(settings)).toBe(true);
  expect(isServerCommand({ action: "new-chat", settings })).toBe(true);
  for (const patch of [
    { id: "other" },
    { previous: "other" },
    { workspaceMode: "shared" },
    { fast_mode: "true" },
    { account_key: null },
    { worker_defaults: [] },
    {
      worker_defaults: {
        model: "x",
        effort: null,
        fast_mode: false,
        id: "other",
      },
    },
    { review_defaults: { model: null } },
  ]) {
    expect(isNewChatSettings({ ...settings, ...patch })).toBe(false);
    expect(
      isServerCommand({
        action: "new-chat",
        settings: { ...settings, ...patch },
      }),
    ).toBe(false);
  }
});

test("switching the lead account resets account-relative model choices", () => {
  const changed = settingsForNewChatAccount(
    {
      account_key: "codex",
      model: "gpt-6-sol",
      effort: "high",
      fast_mode: true,
      worker_defaults: {
        model: "gpt-6-luna",
        effort: "high",
        fast_mode: false,
      },
      review_defaults: { model: "gpt-6-sol", effort: "high" },
    },
    "claude",
  );

  expect(changed).toEqual({
    account_key: "claude",
    model: undefined,
    effort: null,
    fast_mode: false,
    daybreak_enabled: false,
    worker_defaults: { model: "gpt-6-luna", effort: "high", fast_mode: false },
    review_defaults: { model: "gpt-6-sol", effort: "high" },
  });
});

test("new project chat settings send the displayed lead account", () => {
  expect(settingsForNewChatRequest({}, "default", "gpt-6-astra")).toEqual({
    account_key: "default",
    model: "gpt-6-astra",
  });
});

test("new chat settings retain an explicitly chosen account", () => {
  expect(settingsForNewChatRequest({}, "alternate", "gpt-6-astra")).toEqual({
    account_key: "alternate",
    model: "gpt-6-astra",
  });
});

test("a Claude lead request leaves worker defaults to the server unless explicitly chosen", () => {
  const request = settingsForNewChatRequest({}, "claude", "claude-opus-5-5");
  expect(request).toEqual({ account_key: "claude", model: "claude-opus-5-5" });
  expect(request).not.toHaveProperty("worker_defaults");
  const worker_defaults = {
    model: "gpt-6-luna",
    effort: "high",
    fast_mode: false,
  };
  expect(
    settingsForNewChatRequest({ worker_defaults }, "claude", "claude-opus-5-5")
      .worker_defaults,
  ).toEqual(worker_defaults);
});

test("a Layr-only project offers no WT", () => {
  const layrOnly = { workspacePolicy: "layr-native" };
  expect(projectAllowsWorktree(layrOnly)).toBe(false);
});

test("a project without a policy keeps the chosen workspace", () => {
  expect(projectAllowsWorktree(undefined)).toBe(true);
  expect(projectAllowsWorktree({ workspacePolicy: null })).toBe(true);
});

test("new chat rejects a native Layr store override", () => {
  expect(
    isNewChatSettings({
      workspaceMode: "layr-native",
      layrStoreRoot: "/store",
    }),
  ).toBe(false);
  expect(
    isNewChatSettings({ workspaceMode: "layr-native", layrStoreRoot: 42 }),
  ).toBe(false);
});
