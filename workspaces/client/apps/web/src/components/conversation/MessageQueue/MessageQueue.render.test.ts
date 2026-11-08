// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { createStateMirror } from "../../../generated/observableResources.ts";
import { MessageSendStatus } from "../../../generated/api.ts";
import MessageQueue from "./MessageQueue.tsx";

vi.mock("../../../unsent/useUnsentText.ts", () => ({
  useUnsentText: () => [null, vi.fn(), vi.fn()],
}));

it("renders in-place queue item updates without a parent render", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
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
  const mirror = createStateMirror();
  const publish = (text: string) =>
    mirror.apply([
      {
        collection: "queue",
        id: "lead",
        value: {
          items: [
            {
              id: "queued",
              text,
              created: 1,
              status: MessageSendStatus.queued,
              kind: "user",
            },
          ],
          revision: text,
          capabilities: { reorder: true, receipts: true },
        },
      },
    ]);
  publish("Initial queue");
  const queue = mirror.current.queues.get("lead");
  if (!queue) throw new Error("Missing queue fixture");
  const item = queue.items[0];
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const noop = async () => {};
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(MessageQueue, {
            items: queue.items,
            scope: "queue-render",
            onEdit: noop,
            onCancel: noop,
            onReorder: noop,
            canReorder: true,
          }),
        ),
      ),
    );
    expect(host.textContent).toContain("Initial queue");
    await act(async () => publish("Updated queue"));
    expect(queue.items[0]).toBe(item);
    expect(host.textContent).toContain("Updated queue");
    expect(host.textContent).not.toContain("Initial queue");
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});
