// @vitest-environment happy-dom
import { act, createElement, type ComponentProps } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import AccountAddDialog, {
  type AddAccountIntent,
} from "./AccountAddDialog.tsx";
import type AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";
import type ClaudeAddSignIn from "../ClaudeSignIn/ClaudeSignIn.tsx";
import { ServerAreasContextProvider as Provider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { accountsFixture } from "../Accounts/Accounts.fixture.ts";
import { ProviderKind } from "../../../generated/providerKind.ts";

vi.mock("../AccountSignIn/AccountSignIn.tsx", () => ({
  default: ({ label, emailHint }: ComponentProps<typeof AccountSignIn>) =>
    createElement("div", { "data-flow": "codex" }, `${label}|${emailHint}`),
}));
vi.mock("../ClaudeSignIn/ClaudeSignIn.tsx", () => ({
  default: ({ label, email }: ComponentProps<typeof ClaudeAddSignIn>) =>
    createElement("div", { "data-flow": "claude" }, `${label}|${email}`),
}));

it("updates provider and details when intent changes during an open sign-in flow", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);

  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const fixture = accountsFixture();
  const render = async (intent: AddAccountIntent) =>
    act(async () => {
      fixture.ui.openAdd(intent);
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            Provider,
            { value: fixture.areas },
            createElement(AccountAddDialog),
          ),
        ),
      );
    });
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
    fixture.close();
    host.remove();
    vi.unstubAllGlobals();
  }
});
