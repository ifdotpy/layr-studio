// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { act, createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { createRoot } from "react-dom/client";
import type { OutgoingMessage } from "../../../sync/send.ts";
const api = vi.hoisted(() => ({ changeOutbox: vi.fn() }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerOutbox: () => api,
  }),
);
import OutboxControls from "./OutboxControls.tsx";

it("offers no retry after a fresh expired response", () => {
  const html = renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(OutboxControls, {
        entry: {
          id: "one",
          body: { id: "one", room: "chat", text: "hello" },
          created: 1,
          status: {
            source: "device",
            state: { kind: "receipt_expired", error: "expired" },
          },
        },
      }),
    ),
  );
  expect(html).toContain("Discard");
  expect(html).not.toContain("Retry");
});

it("keeps the original send locked while its controls hide and return", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const entry: OutgoingMessage = {
    id: "same-message",
    body: { id: "same-message", room: "chat", text: "hello" },
    created: 1,
    status: {
      source: "device",
      state: { kind: "transport_failed", error: "offline" },
    },
  };
  const host = document.createElement("div"),
    root = createRoot(host);
  let finish = () => {};
  api.changeOutbox.mockReset().mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const show = (entry: OutgoingMessage) =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(OutboxControls, { entry }),
        ),
      ),
    );
  try {
    await show(entry);
    await act(async () => {
      host.querySelector<HTMLButtonElement>("button")!.click();
      host.querySelector<HTMLButtonElement>("button")!.click();
    });
    await show({
      ...entry,
      status: { source: "device", state: { kind: "in_flight" } },
    });
    expect(host.querySelector("button")).toBeNull();
    await show(entry);
    expect(host.querySelector<HTMLButtonElement>("button")?.disabled).toBe(
      true,
    );
    expect(api.changeOutbox).toHaveBeenCalledExactlyOnceWith(
      "same-message",
      "resume",
    );
    await act(async () => finish());
    expect(host.querySelector<HTMLButtonElement>("button")?.disabled).toBe(
      false,
    );
  } finally {
    await act(async () => root.unmount());
    vi.unstubAllGlobals();
  }
});
