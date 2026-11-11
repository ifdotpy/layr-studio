// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { AccountStatus, ProviderKind } from "../../../generated/api.ts";
import type { Account } from "../../accounts/support/Accounts.tsx";
import { observableFixture } from "../integration-support/observableFixture.tsx";
import ClaudeProfile from "./ClaudeProfile.tsx";
const live = vi.hoisted(() => ({ post: vi.fn(), serverId: "server-a" }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => ({ post: live.post }),
    useServerOwner: () => ({ id: live.serverId }),
  }),
);
const account = (id: string, launchArgs: string): Account => ({
  id,
  label: id,
  status: AccountStatus.ready,
  provider: ProviderKind.claude,
  home: "/fixture",
  source: "Fixture",
  claudeOptions: { launchArgs },
});
it("resets drafts across account and server identity while using current same-target API and callback", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: new EventTarget(),
  });
  const f = observableFixture();
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const oldSaved = vi.fn(),
    newSaved = vi.fn();
  const oldPost = vi.fn().mockResolvedValue({ ok: true });
  const newPost = vi.fn().mockResolvedValue({ ok: true });
  const render = (target: Account, onSaved = oldSaved) =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            f.Provider,
            null,
            createElement(ClaudeProfile, {
              account: target,
              onSaved,
              expanded: true,
            }),
          ),
        ),
      ),
    );
  const args = () =>
    host.querySelector<HTMLInputElement>(
      'input[placeholder="--add-dir /path/to/project"]',
    )!;
  const submit = () =>
    act(async () =>
      host
        .querySelector("form")!
        .dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        ),
    );
  try {
    live.post = oldPost;
    live.serverId = "server-a";
    await render(account("a", "--from-a"));
    expect(args().value).toBe("--from-a");
    await render(account("b", "--from-b"));
    expect(args().value).toBe("--from-b");
    live.post = newPost;
    await render(account("b", "--streamed-b"), newSaved);
    expect(args().value).toBe("--from-b");
    await submit();
    expect(oldPost).not.toHaveBeenCalled();
    expect(newPost).toHaveBeenLastCalledWith(
      "/api/claude/profiles",
      expect.objectContaining({
        account_key: "b",
        options: expect.objectContaining({ launchArgs: "--from-b" }),
      }),
    );
    expect(oldSaved).not.toHaveBeenCalled();
    expect(newSaved).toHaveBeenCalledTimes(1);
    live.serverId = "server-b";
    await render(account("b", "--server-b"));
    expect(args().value).toBe("--server-b");
  } finally {
    await act(async () => root.unmount());
    f.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});
