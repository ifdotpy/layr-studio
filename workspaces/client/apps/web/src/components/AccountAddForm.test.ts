// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { runInAction } from "mobx";
import { expect, it, vi } from "vitest";
import {
  AccountAddActionsContext,
  AccountAddForm,
  createAccountAddDraft,
} from "./AccountAddForm";
import { ProviderKind } from "../generated/providerKind";
import { LOGIN_LABEL_MAX_CHARS } from "../generated/sharedValues";

it("reads the draft directly and submits through its actions context", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const model = createAccountAddDraft();
  const actions = { onContinue: vi.fn(), onCancel: vi.fn() };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            AccountAddActionsContext,
            { value: actions },
            createElement(AccountAddForm, {
              model,
              children: createElement("span", null, "Server selector"),
            }),
          ),
        ),
      ),
    );
    const continueButton = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Continue",
    );
    expect(continueButton?.disabled).toBe(true);
    await act(async () =>
      runInAction(() => {
        model.provider = ProviderKind.Claude;
        model.label = "Work";
      }),
    );
    expect(continueButton?.disabled).toBe(false);
    expect(
      host
        .querySelector<HTMLInputElement>("input[maxlength]")
        ?.getAttribute("maxlength"),
    ).toBe(String(LOGIN_LABEL_MAX_CHARS));
    expect(
      host.querySelector<HTMLInputElement>("input[maxlength]")?.value,
    ).toBe("Work");
    await act(async () => continueButton?.click());
    expect(actions.onContinue).toHaveBeenCalledOnce();
    expect(host.textContent).toContain("Server selector");
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});
