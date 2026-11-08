// @vitest-environment happy-dom
import { act, createElement, type ComponentProps } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import AccountAddDialog, {
  type AddAccountIntent,
} from "./AccountAddDialog.tsx";
import type AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";
import type ClaudeAddSignIn from "../ClaudeAddSignIn/ClaudeAddSignIn.tsx";
import { ProviderKind } from "../../../generated/providerKind.ts";

vi.mock("../AccountSignIn/AccountSignIn.tsx", () => ({
  default: ({ label, emailHint }: ComponentProps<typeof AccountSignIn>) =>
    createElement("div", { "data-flow": "codex" }, `${label}|${emailHint}`),
}));
vi.mock("../ClaudeAddSignIn/ClaudeAddSignIn.tsx", () => ({
  default: ({ label, email }: ComponentProps<typeof ClaudeAddSignIn>) =>
    createElement("div", { "data-flow": "claude" }, `${label}|${email}`),
}));

it("updates provider and details when intent changes during an open sign-in flow", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const state = {
    data: {
      accounts: [],
      archivedAccounts: [],
      defaultAccountKey: "default",
      defaultAccountGroups: [],
      logins: [],
      supportsDelete: true,
      supportsDisconnect: true,
    },
    setData: vi.fn(),
    error: "",
    refresh: vi.fn(async () => null),
    scope: "local",
  } satisfies ComponentProps<typeof AccountAddDialog>["state"];
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const render = async (intent: AddAccountIntent) =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(AccountAddDialog, {
            opened: true,
            intent,
            state,
            onClose: vi.fn(),
          }),
        ),
      ),
    );
  try {
    await render({
      provider: ProviderKind.Codex,
      label: "First",
      email: "first@example.test",
    });
    expect(document.querySelector('[data-flow="codex"]')?.textContent).toBe(
      "First|first@example.test",
    );
    await render({
      provider: ProviderKind.Codex,
      label: "Second",
      email: "second@example.test",
    });
    expect(document.querySelector('[data-flow="codex"]')?.textContent).toBe(
      "Second|second@example.test",
    );
    await render({
      provider: ProviderKind.Claude,
      label: "Claude work",
      email: "work@example.test",
    });
    expect(document.querySelector('[data-flow="codex"]')).toBeNull();
    expect(document.querySelector('[data-flow="claude"]')?.textContent).toBe(
      "Claude work|work@example.test",
    );
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});
