// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { observable, runInAction } from "mobx";
import { expect, it, vi } from "vitest";
import PromptNavigator from "./PromptNavigator.tsx";
import { MessageSendStatus } from "../../../generated/api.ts";
import type { Message } from "../../../types.ts";

const probe = vi.hoisted(() => ({ renders: 0 }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerApi: () => {
      probe.renders++;
      return { syncGet: vi.fn() };
    },
  }),
);
vi.mock("../../../sync/useSyncedVisualState.ts", async () => {
  const { useState } = await import("react");
  return { useSyncedVisualState: () => useState({}) };
});

it("isolates transcript changes and scroll-active markers from the navigator", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: () => true,
  }));
  const frames = new Map<number, FrameRequestCallback>();
  let frameId = 0;
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    frames.set(++frameId, callback);
    return frameId;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
  const host = document.createElement("div");
  const transcript = document.createElement("div");
  const messages = observable<Message[]>([
    { id: "one", role: "user", text: "First" },
    { id: "answer", role: "assistant", text: "Answer" },
    { id: "two", role: "user", text: "Second" },
    { id: "three", role: "user", text: "Third" },
  ]);
  let position = 0;
  for (const [index, id] of ["one", "two", "three"].entries()) {
    const element = document.createElement("div");
    element.dataset.message = id;
    element.getBoundingClientRect = () =>
      new DOMRect(0, index * 100 - position, 100, 50);
    transcript.append(element);
  }
  document.body.append(host, transcript);
  const root = createRoot(host);
  const props = {
    messages,
    container: { current: transcript },
    storageKey: "test",
    jump: vi.fn(),
  };
  const render = () =>
    root.render(
      createElement(
        MantineProvider,
        null,
        createElement(PromptNavigator, props),
      ),
    );
  const flush = async () => {
    await act(async () => {
      const pending = [...frames.values()];
      frames.clear();
      pending.forEach((callback) => callback(0));
    });
  };
  try {
    await act(async () => render());
    await flush();
    const mounted = probe.renders;
    await act(async () => {
      runInAction(() => {
        messages[1]!.text += " streamed";
      });
      render(); // An unrelated Conversation render with unchanged navigator inputs.
    });
    await act(async () => {
      runInAction(() => {
        messages.push({ id: "tool", role: "assistant", text: "Tool result" });
      });
    });
    const transcriptRenders = probe.renders - mounted;
    const beforeScroll = probe.renders;
    for (position of [100, 200, 0]) {
      transcript.dispatchEvent(new Event("scroll"));
      await flush();
      expect(
        host.querySelector(".prompt-history-toggle-label")?.textContent,
      ).toBe(`${position / 100 + 1} / 3`);
      expect(host.querySelector(".current-prompt")?.textContent).toBe(
        ["First", "Second", "Third"][position / 100],
      );
    }
    const scrollRenders = probe.renders - beforeScroll;
    console.log({ transcriptRenders, scrollRenders });
    expect(
      host.querySelector(".prompt-history-toggle-label")?.textContent,
    ).toBe("1 / 3");
    expect(transcriptRenders).toBe(0);
    expect(scrollRenders).toBe(0);
    await act(async () => {
      runInAction(() => {
        messages[0]!.text = "Edited first";
      });
    });
    expect(host.querySelector(".current-prompt")?.textContent).toBe(
      "Edited first",
    );
    await act(async () => {
      runInAction(() => {
        messages.push({
          id: "pending",
          role: "user",
          text: "Pending",
          deliveryStatus: {
            source: "server",
            status: MessageSendStatus.queued,
          },
        });
      });
    });
    expect(
      host.querySelector(".prompt-history-toggle-label")?.textContent,
    ).toBe("1 / 3");
    await act(async () => {
      runInAction(() => {
        messages[messages.length - 1]!.deliveryStatus = undefined;
      });
    });
    expect(
      host.querySelector(".prompt-history-toggle-label")?.textContent,
    ).toBe("1 / 4");
    await act(async () => {
      host.querySelector<HTMLButtonElement>(".current-prompt")!.click();
    });
    expect(props.jump).toHaveBeenCalledWith("one");
  } finally {
    await act(async () => root.unmount());
    host.remove();
    transcript.remove();
    vi.unstubAllGlobals();
  }
});
