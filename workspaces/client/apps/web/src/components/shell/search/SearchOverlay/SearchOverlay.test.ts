// @vitest-environment happy-dom
import { act, createElement, StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { SearchContent } from "../SearchContent/SearchContent.tsx";
import { createServerStore } from "../../../../store/server.ts";
import { createSubscriptions } from "../../../../sync/subscriptions/module.ts";
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
const fixture = vi.hoisted(() => ({
  get: vi.fn(),
  result: { id: "message", agent: "same", text: "Excerpt", kind: "message" },
}));
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerApi: () => ({
      get: fixture.get,
      getStreamSession: () => undefined,
    }),
    useServerStore: () => ({
      reads: store.reads,
      state: () => ({
        current: {
          status: "live",
          data: { agents: new Map([["same", { name: "Own server" }]]) },
        },
      }),
    }),
  }),
);
const store = createServerStore(
  createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: () => () => {},
  }),
);
it("renders scoped names and ignores a source reply after its detail is closed", async () => {
  const pending = Promise.withResolvers<unknown>();
  fixture.get
    .mockResolvedValueOnce({ results: [fixture.result] })
    .mockReturnValueOnce(pending.promise);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const source = {
    opened: true,
    close: vi.fn(),
    select: vi.fn(),
    navigate: vi.fn(),
  };
  await act(async () =>
    root.render(
      createElement(
        MantineProvider,
        null,
        createElement(
          StrictMode,
          null,
          createElement(SearchContent, { source }),
        ),
      ),
    ),
  );
  expect(fixture.get).not.toHaveBeenCalled();
  const input = host.querySelector<HTMLInputElement>("input")!;
  await act(async () => {
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )!.set!.call(input, "query");
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await act(async () =>
    host
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })),
  );
  expect(fixture.get).toHaveBeenCalledTimes(1);
  expect(host.textContent).toContain("Own server");
  await act(async () =>
    host.querySelector<HTMLButtonElement>(".workspace-row")!.click(),
  );
  const detail = document.querySelector<HTMLElement>('[role="dialog"]')!;
  expect(detail.textContent).toContain("Search source");
  await act(async () =>
    detail
      .querySelector<HTMLButtonElement>("button.mantine-Modal-close")!
      .click(),
  );
  await act(async () => {
    pending.resolve({ ...fixture.result, text: "Late full source" });
    await pending.promise;
  });
  expect(document.body.textContent).not.toContain("Late full source");
  expect(source.select).not.toHaveBeenCalled();
  await act(async () => root.unmount());
  host.remove();
});
