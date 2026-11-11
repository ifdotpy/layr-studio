// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { dictationLanguageOptions } from "./BrowserDictation.tsx";

it("reuses language names until the device locale changes", () => {
  const displayNames = vi.spyOn(Intl, "DisplayNames");
  try {
    vi.stubGlobal("navigator", { language: "en-US" });
    const english = dictationLanguageOptions();
    dictationLanguageOptions();
    expect(displayNames).toHaveBeenCalledTimes(1);
    expect(
      english.filter((option) => option.props.value === "en-US"),
    ).toHaveLength(1);
    expect(
      english.find((option) => option.props.value === "de-DE")?.props.children,
    ).toBe("German (Germany)");
    vi.stubGlobal("navigator", { language: "fr-FR" });
    const french = dictationLanguageOptions();
    expect(displayNames).toHaveBeenCalledTimes(2);
    expect(
      french.find((option) => option.props.value === "de-DE")?.props.children,
    ).toBe("allemand (Allemagne)");
  } finally {
    displayNames.mockRestore();
    vi.unstubAllGlobals();
  }
});

// Browser speech callbacks exercise the shared owner through its actual view.
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { BrowserDictation, type Recognition } from "./BrowserDictation.tsx";
import { createBrowserDictation } from "./BrowserDictationStore.ts";
import { DictationStore } from "../DictationPanel/DictationPanel.tsx";
const fixture = vi.hoisted(() => ({
  owner: undefined as ReturnType<typeof createBrowserDictation> | undefined,
}));
vi.mock("../support/ConversationStore.ts", () => ({
  useConversationStore: () => ({
    context: { browserDictation: fixture.owner },
  }),
}));

it("retains review text after uncertain final words and tears down speech on chat switch", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal("isSecureContext", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
  }));
  const speech: { current?: Recognition } = {};
  const starts = vi.fn(),
    stops = vi.fn(),
    aborts = vi.fn();
  class SpeechRecognition implements Recognition {
    lang = "";
    continuous = false;
    interimResults = false;
    onresult: Recognition["onresult"] = null;
    onerror: Recognition["onerror"] = null;
    onstart: Recognition["onstart"] = null;
    onend: Recognition["onend"] = null;
    constructor() {
      speech.current = this;
    }
    start() {
      starts();
      this.onstart?.();
    }
    stop() {
      stops();
    }
    abort() {
      aborts();
    }
  }
  vi.stubGlobal("SpeechRecognition", SpeechRecognition);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const insert = vi.fn();
  let previous = "";
  const render = async (chatId: string) =>
    act(async () => {
      if (previous !== chatId) {
        fixture.owner?.dispose();
        fixture.owner = createBrowserDictation(new DictationStore());
        previous = chatId;
      }
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(BrowserDictation, { chatId, onInsert: insert }),
        ),
      );
    });
  const button = (text: string) => {
    const found = [
      ...document.querySelectorAll<HTMLButtonElement>("button"),
    ].find((button) => button.textContent?.trim() === text);
    if (!found) throw Error(`Missing button ${text}`);
    return found;
  };
  try {
    await render("first");
    await act(async () =>
      host
        .querySelector<HTMLButtonElement>('[aria-label="Dictation"]')
        ?.click(),
    );
    await act(async () => {
      button("Record").click();
      expect(starts).toHaveBeenCalledOnce();
    });
    if (!speech.current) throw Error("missing recognition");
    const oldRecognition = speech.current;
    await act(async () =>
      oldRecognition.onresult?.({
        results: [
          { isFinal: true, 0: { transcript: "saved words" } },
          { isFinal: false, 0: { transcript: "unfinished" } },
        ],
      }),
    );
    const review = document.querySelector<HTMLTextAreaElement>(
      '[aria-label="Review dictated text"]',
    );
    expect(review?.value).toBe("saved words");
    expect(review?.disabled).toBe(true);
    expect(button("Insert into message").disabled).toBe(true);
    vi.useFakeTimers();
    await act(async () => button("Stop dictation").click());
    await act(async () => vi.advanceTimersByTimeAsync(5000));
    expect(aborts).toHaveBeenCalledOnce();
    expect(document.body.textContent).toContain(
      "did not return the final words",
    );
    expect(review?.value).toBe("saved words");
    expect(button("Insert into message").disabled).toBe(false);
    expect(insert).not.toHaveBeenCalled();
    await act(async () => button("Record").click());
    await act(async () => button("Stop dictation").click());
    await render("second");
    expect(aborts).toHaveBeenCalledTimes(2);
    await act(async () =>
      oldRecognition.onresult?.({
        results: [{ isFinal: true, 0: { transcript: "late words" } }],
      }),
    );
    expect(button("Record").disabled).toBe(false);
    expect(review?.disabled).toBe(false);
    await act(async () => button("Record").click());
    expect(starts).toHaveBeenCalledTimes(3);
    await act(async () => vi.advanceTimersByTimeAsync(6000));
    expect(aborts).toHaveBeenCalledTimes(2);
    expect(review?.value).toBe("");
  } finally {
    await act(async () => {
      fixture.owner?.dispose();
      root.unmount();
    });
    host.remove();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  }
});
