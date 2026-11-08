// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, createElement, createRef } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import PromptInput from "./PromptInput";

for (const mobile of [true, false])
  for (const laterNative of [true, false])
    it.each([
      ["incoming draft", "External draft"],
      ["send reset", ""],
      ["restore", "Restored draft"],
    ])(
      `orders %s during composition (mobile=${mobile}, laterNative=${laterNative})`,
      async (_name, external) => {
        vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
        vi.stubGlobal(
          "ResizeObserver",
          class {
            observe() {}
            disconnect() {}
          },
        );
        const input = createRef<HTMLTextAreaElement>();
        const change = vi.fn();
        const host = document.body.appendChild(document.createElement("div"));
        const root = createRoot(host);
        const render = (
          value: string,
          draftTooLong = false,
          draftResetVersion = 0,
        ) =>
          act(async () =>
            root.render(
              createElement(
                MantineProvider,
                null,
                createElement(PromptInput, {
                  value,
                  draftResetVersion,
                  session: "chat",
                  getDraft: () => value,
                  setDraft: change,
                  input,
                  mobile,
                  managed: true,
                  canSend: true,
                  blocked: false,
                  sending: false,
                  uploading: false,
                  draftTooLong,
                  modelCommand: false,
                  hasAttachments: false,
                  onChange: change,
                  onPasteFiles: () => {},
                  onSend: () => {},
                  onQueue: () => {},
                  onRecallKeyDown: () => false,
                  skillCatalog: {
                    enabled: false,
                    agentId: "chat",
                    workspace: "workspace",
                    account: "account",
                    cwd: "/project",
                    provider: "codex",
                  },
                }),
              ),
            ),
          );
        try {
          await render("Draft");
          const element = input.current;
          if (!element) throw new Error("Missing textarea");
          element.focus();
          await act(async () =>
            element.dispatchEvent(
              new CompositionEvent("compositionstart", { bubbles: true }),
            ),
          );
          // The IME updates live value without changing the textarea's child text.
          element.value = "Draft編";
          const backingText = element.textContent;
          const write = vi.spyOn(element, "value", "set");
          const selection = vi.spyOn(element, "setSelectionRange");
          const focus = vi.spyOn(element, "focus");
          const blur = vi.spyOn(element, "blur");
          await render("Draft編");
          expect(element.textContent).toBe(backingText);
          const reset = _name === "send reset";
          await render(external, true, reset ? 1 : 0);
          expect(input.current).toBe(element);
          expect(document.activeElement).toBe(element);
          expect(element.value).toBe("Draft編");
          expect(write).not.toHaveBeenCalled();
          expect(selection).not.toHaveBeenCalled();
          expect(focus).not.toHaveBeenCalled();
          expect(blur).not.toHaveBeenCalled();
          if (laterNative) {
            const descriptor = Object.getOwnPropertyDescriptor(
              HTMLTextAreaElement.prototype,
              "value",
            );
            if (!descriptor?.set)
              throw new Error("Missing native value setter");
            await act(async () => {
              descriptor.set!.call(element, "Draft編集");
              element.dispatchEvent(
                new InputEvent("input", {
                  bubbles: true,
                  inputType: "insertCompositionText",
                  data: "集",
                  isComposing: true,
                }),
              );
            });
          }
          await act(async () =>
            element.dispatchEvent(
              new CompositionEvent("compositionend", {
                bubbles: true,
                data: "編",
              }),
            ),
          );
          const nativeWins = laterNative && !reset;
          const expected = nativeWins ? "Draft編集" : external;
          expect(element.value).toBe(expected);
          if (nativeWins) expect(change).toHaveBeenLastCalledWith(expected);
          else expect(change).not.toHaveBeenCalled();
          await render(expected, false, reset ? 1 : 0);
          expect(write).toHaveBeenCalledTimes(nativeWins ? 0 : 1);
          await render("Next restored draft");
          expect(element.value).toBe("Next restored draft");
          expect(write).toHaveBeenCalledTimes(nativeWins ? 1 : 2);
        } finally {
          await act(async () => root.unmount());
          host.remove();
          vi.restoreAllMocks();
          vi.unstubAllGlobals();
        }
      },
    );

for (const beforeSelection of [true, false])
  it(`does not move the caret when composition starts ${beforeSelection ? "before autocomplete selection" : "before its queued frame"}`, async () => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        disconnect() {}
      },
    );
    const frames = new Map<number, FrameRequestCallback>();
    let nextFrame = 0;
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      frames.set(++nextFrame, callback);
      return nextFrame;
    });
    vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
    const input = createRef<HTMLTextAreaElement>();
    const change = vi.fn();
    const host = document.body.appendChild(document.createElement("div"));
    const root = createRoot(host);
    try {
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            null,
            createElement(PromptInput, {
              value: "/ren",
              session: "chat",
              getDraft: () => "/ren",
              setDraft: change,
              input,
              mobile: true,
              managed: true,
              canSend: true,
              blocked: false,
              sending: false,
              uploading: false,
              draftTooLong: false,
              modelCommand: false,
              hasAttachments: false,
              onChange: change,
              onPasteFiles: () => {},
              onSend: () => {},
              onQueue: () => {},
              onRecallKeyDown: () => false,
              skillCatalog: {
                enabled: false,
                agentId: "chat",
                workspace: "workspace",
                account: "account",
                cwd: "/project",
                provider: "codex",
              },
            }),
          ),
        ),
      );
      const element = input.current;
      const option = document.querySelector<HTMLElement>('[role="option"]');
      if (!element || !option)
        throw new Error("Missing autocomplete input or option");
      element.focus();
      const focus = vi.spyOn(element, "focus");
      const selection = vi.spyOn(element, "setSelectionRange");
      await act(async () => {
        const start = () =>
          element.dispatchEvent(
            new CompositionEvent("compositionstart", { bubbles: true }),
          );
        if (beforeSelection) start();
        option.click();
        if (!beforeSelection) start();
      });
      expect(change).toHaveBeenCalledWith("/rename ", "chat");
      await act(async () => {
        for (const callback of frames.values()) callback(0);
        frames.clear();
      });
      expect(focus).not.toHaveBeenCalled();
      expect(selection).not.toHaveBeenCalled();
    } finally {
      await act(async () => root.unmount());
      host.remove();
      vi.restoreAllMocks();
      vi.unstubAllGlobals();
    }
  });
