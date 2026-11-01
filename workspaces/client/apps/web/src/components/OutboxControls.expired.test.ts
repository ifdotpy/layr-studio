import { expect, it, vi } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
vi.mock("../servers/ServerServicesContext", () => ({
  useServerOutbox: () => ({ changeOutbox: vi.fn() }),
}));
import OutboxControls from "./OutboxControls";

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
        edit: async () => {},
      }),
    ),
  );
  expect(html).toContain("Discard");
  expect(html).not.toContain("Retry");
});
