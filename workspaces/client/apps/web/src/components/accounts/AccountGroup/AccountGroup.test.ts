import { ProviderKind, AccountStatus } from "../../../generated/api.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, test, vi } from "vitest";
import AccountGroup from "./AccountGroup.tsx";
import type { PostResult } from "../../../api.ts";
import type { Account } from "../support/Accounts.tsx";

const api = vi.hoisted(() => ({ post: vi.fn() }));
const post = api.post;
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerApi: () => api,
  }),
);

test("the group editor validates, locks one save and reports its result", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  const onSaved = vi.fn();
  const result = {
    accounts: [],
    archivedAccounts: [],
    defaultAccountGroups: [],
    defaultAccountKey: "",
    logins: [],
    supportsDelete: true,
    supportsDisconnect: true,
  } satisfies PostResult<"/api/accounts/group">;
  const account = {
    id: "work",
    label: "Work",
    provider: ProviderKind.codex,
    status: AccountStatus.ready,
    home: "",
    source: "fixture",
  } satisfies Account;
  const input = () => {
    const field = host.querySelector("input");
    if (!field) throw new Error("Group field missing");
    return field;
  };
  const edit = async (value: string) =>
    act(async () => {
      Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )?.set?.call(input(), value);
      input().dispatchEvent(new Event("input", { bubbles: true }));
    });
  const submit = () =>
    host
      .querySelector("form")
      ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(AccountGroup, {
            account,
            serverLabel: "Remote",
            onSaved,
          }),
        ),
      ),
    );
    expect(input().getAttribute("aria-label")).toBe("Group of Work on Remote");
    await edit("personal, work");
    await act(async () => {
      submit();
    });
    expect(post).not.toHaveBeenCalled();
    await edit("personal");
    let finish: (value: typeof result) => void = () => {};
    post.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    await act(async () => {
      submit();
      submit();
    });
    expect(post).toHaveBeenCalledTimes(1);
    expect(post).toHaveBeenCalledWith("/api/accounts/group", {
      account_key: "work",
      group: "personal",
    });
    expect(input().disabled).toBe(true);
    await act(async () => finish(result));
    expect(onSaved).toHaveBeenCalledWith(result);
    expect(host.textContent).toContain("Group saved.");
    await edit("other");
    expect(host.textContent).not.toContain("Group saved.");
    post.mockRejectedValueOnce(new Error("Offline"));
    await act(async () => {
      submit();
    });
    expect(host.querySelector('[role="alert"]')?.textContent).toBe("Offline");
    expect(input().disabled).toBe(false);
  } finally {
    await act(async () => root.unmount());
    vi.unstubAllGlobals();
  }
});

test("a mounted group editor uses the current target, API and callback", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  const oldSaved = vi.fn(),
    newSaved = vi.fn();
  const base = {
    id: "first",
    label: "First",
    provider: ProviderKind.codex,
    status: AccountStatus.ready,
    home: "",
    source: "fixture",
    group: "original",
  } satisfies Account;
  const result = {
    accounts: [],
    archivedAccounts: [],
    defaultAccountGroups: [],
    defaultAccountKey: "",
    logins: [],
    supportsDelete: true,
    supportsDisconnect: true,
  } satisfies PostResult<"/api/accounts/group">;
  const newPost = vi.fn().mockResolvedValue(result);
  const render = async (account: Account, onSaved = oldSaved) =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(AccountGroup, { account, onSaved }),
        ),
      ),
    );
  const input = () => {
    const field = host.querySelector("input");
    if (!field) throw new Error("Group field missing");
    return field;
  };
  const edit = async (value: string) =>
    act(async () => {
      Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )?.set?.call(input(), value);
      input().dispatchEvent(new Event("input", { bubbles: true }));
    });
  try {
    await render(base);
    await edit("abandoned");
    await render({ ...base, id: "second", group: "personal" });
    expect(input().value).toBe("personal");
    await render({ ...base, id: "second", group: "updated" });
    expect(input().value).toBe("updated");
    await edit("work");
    api.post = newPost;
    await render(
      { ...base, id: "second", label: "Current", group: "updated" },
      newSaved,
    );
    expect(input().getAttribute("aria-label")).toBe("Group of Current");
    await act(async () => {
      host
        .querySelector("form")
        ?.dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        );
    });
    expect(newPost).toHaveBeenCalledWith("/api/accounts/group", {
      account_key: "second",
      group: "work",
    });
    expect(newSaved).toHaveBeenCalledWith(result);
    expect(oldSaved).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
    api.post = post;
    vi.unstubAllGlobals();
  }
});
