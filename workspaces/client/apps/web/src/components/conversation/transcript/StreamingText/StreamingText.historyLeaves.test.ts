// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { afterEach, expect, it, vi } from "vitest";
import { createTranscriptMirror } from "../../../../generated/observableResources.ts";
import { createMessageViews } from "../../../../conversation/messageView.ts";
import { createLiveText } from "../../../../conversation/liveText.ts";
import { observableFixture } from "../../../shell/integration-support/observableFixture.tsx";
import TurnHistory from "../TurnHistory/TurnHistory.fixture.tsx";
import StreamingText from "./StreamingText.tsx";
import type { Message } from "../../../../types.ts";

vi.mock("../../markdown/NativeMarkdown/NativeMarkdown.tsx", () => ({
  default: ({ text }: { text: string }) =>
    createElement("span", { "data-text-leaf": true }, text),
}));
afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

it("real TurnHistory mounts an empty streaming leaf, shows its first live token before durable text, then hides an empty finished row", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const mirror = createTranscriptMirror({
    items: [{ id: "live", role: "assistant", text: "", streaming: true }],
    truncated: false,
  });
  const views = createMessageViews("agent");
  const history = { live: createLiveText() };
  const models = observableFixture();
  const Row = observer(function Row({ item }: { item: Message }) {
    return createElement(StreamingText, { message: item });
  });
  const View = observer(function View() {
    return createElement(TurnHistory, {
      items: views(mirror.current.items),
      history,
      enabled: false,
      storageKey: "live-visibility",
      renderMessage: (item) => createElement(Row, { item }),
      onJump: () => {},
    });
  });
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(createElement(models.Provider, null, createElement(View))),
    );
    expect(host.querySelector("[data-text-leaf]")).not.toBeNull();
    await act(async () => {
      history.live.add({ itemId: "live", offset: 0, text: "first token" });
    });
    expect(mirror.current.items[0]?.text).toBe("");
    expect(host.textContent).toContain("first token");
    await act(async () => {
      history.live.clear();
      mirror.apply({
        items: [{ id: "live", role: "assistant", text: "", streaming: false }],
        truncated: false,
      });
    });
    expect(host.querySelector("[data-text-leaf]")).toBeNull();
  } finally {
    await act(async () => root.unmount());
    models.store.dispose();
  }
});

it("real TurnHistory observes an in-place terminal outcome without replacing its row reference", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const mirror = createTranscriptMirror({
    items: [
      {
        id: "answer",
        role: "assistant",
        text: "answer",
        turnId: "turn",
        streaming: true,
      },
    ],
    truncated: false,
  });
  const items = createMessageViews("agent")(mirror.current.items);
  const models = observableFixture();
  const View = observer(function View() {
    return createElement(TurnHistory, {
      items,
      enabled: true,
      storageKey: "mutable-outcome",
      renderMessage: (item) => createElement(StreamingText, { message: item }),
      onJump: () => {},
    });
  });
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(createElement(models.Provider, null, createElement(View))),
    );
    expect(host.querySelector('[data-outcome="active"]')).not.toBeNull();
    await act(async () =>
      mirror.apply({
        items: [
          {
            id: "answer",
            role: "assistant",
            text: "answer",
            turnId: "turn",
            turnStatus: "completed",
            streaming: false,
          },
        ],
        truncated: false,
      }),
    );
    expect(host.querySelector('[data-outcome="completed"]')).not.toBeNull();
  } finally {
    await act(async () => root.unmount());
    models.store.dispose();
  }
});
