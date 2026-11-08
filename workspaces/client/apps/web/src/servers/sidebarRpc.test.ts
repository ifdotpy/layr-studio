import { afterEach, expect, it, vi } from "vitest";
const post = vi.hoisted(() => vi.fn());
vi.mock("../api", async (original) => ({
  ...(await original<typeof import("../api")>()),
  post,
}));
import { executeSidebarRequest } from "./sidebarRpc";
import { navigationSnapshot } from "./navigation";

afterEach(() => {
  post.mockReset();
  vi.useRealTimers();
});
it("uses the frame API with the exact request identity and options", async () => {
  const body = {
    action: "rename",
    path: "/remote",
    name: "Draft",
    expected_revision: 9,
    request_id: "same-request",
  };
  post.mockResolvedValue({ revision: 10 });
  const refresh = vi.fn();
  const run = vi.fn();
  expect(
    await executeSidebarRequest(
      {
        action: "post",
        path: "/api/projects",
        body,
        options: { timeoutMs: 15000, requestId: "same-request" },
      },
      refresh,
      run,
      "owner-session-token",
    ),
  ).toEqual({ revision: 10 });
  expect(post).toHaveBeenCalledExactlyOnceWith("/api/projects", body, {
    timeoutMs: 15000,
    requestId: "same-request",
    sessionToken: "owner-session-token",
  });
});

it("does not post a sidebar mutation without the owning frame session token", async () => {
  await expect(
    executeSidebarRequest(
      {
        action: "post",
        path: "/api/projects",
        body: { action: "reorder", request_id: "order-without-token" },
      },
      vi.fn(),
      vi.fn(),
    ),
  ).rejects.toThrow("The sidebar server session is not ready.");
  expect(post).not.toHaveBeenCalled();
});

it("denies non-sidebar API paths, credential options and unrelated commands", async () => {
  const refresh = vi.fn();
  const run = vi.fn();
  for (const value of [
    { action: "post", path: "/api/messages", body: { text: "No" } },
    {
      action: "post",
      path: "/api/rename",
      body: {},
      options: { sessionToken: "forbidden" },
    },
    {
      action: "post",
      path: "/api/rename",
      body: {},
      options: { timeoutMs: 60000 },
    },
    {
      action: "command",
      command: { action: "account-sign-in", provider: "codex" },
    },
  ])
    await expect(executeSidebarRequest(value, refresh, run)).rejects.toThrow(
      "Invalid sidebar",
    );
  expect(post).not.toHaveBeenCalled();
  expect(run).not.toHaveBeenCalled();
});

it("runs prefetch and unread callbacks in the owning frame and returns its fresh navigation", async () => {
  const navigation = navigationSnapshot(undefined, null, "");
  const refresh = vi.fn(async () => navigation);
  const run = vi.fn(async () => {});
  const command = {
    action: "mark-unread",
    id: "same",
    target: { threadId: "thread", turnId: "turn", read: true, revision: 7 },
  };
  await executeSidebarRequest({ action: "command", command }, refresh, run);
  expect(run).toHaveBeenCalledWith(command);
  await executeSidebarRequest(
    { action: "command", command: { action: "prepare-chat", id: "same" } },
    refresh,
    run,
  );
  expect(run).toHaveBeenCalledWith({ action: "prepare-chat", id: "same" });
  expect(
    await executeSidebarRequest({ action: "refresh" }, refresh, run),
  ).toEqual(navigation);
  expect(post).not.toHaveBeenCalled();
});

it("organization replies expose only the folder action receipt", async () => {
  post.mockResolvedValue({
    id: "same",
    pinned: true,
    projectFolder: "nested",
    projectFolderRevision: 7,
    prompt: "PRIVATE_PROMPT",
    lastAnswer: "PRIVATE_ANSWER",
    runtime: { token: "PRIVATE_TOKEN" },
  });
  const result = await executeSidebarRequest(
    {
      action: "post",
      path: "/api/organization",
      body: { id: "same", pinned: true },
    },
    vi.fn(),
    vi.fn(),
    "owner-session-token",
  );
  expect(result).toEqual({ projectFolder: "nested", projectFolderRevision: 7 });
});
