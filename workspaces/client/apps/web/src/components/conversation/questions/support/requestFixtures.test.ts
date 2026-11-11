// @vitest-environment happy-dom
import { renderToStaticMarkup } from "react-dom/server";
import { StrictMode, act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { enableStaticRendering } from "mobx-react-lite";
import { getObserverTree } from "mobx";
import { afterEach, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import { RequestFixture, createRequestFixture } from "./requestFixtures.ts";
import { RequestAreaOwner } from "../RequestArea/RequestArea.tsx";
import Requests from "../Requests/Requests.tsx";
import { approval } from "../../../../approvalFixtures.ts";
import { requestTestEnvironment } from "../Requests/requestTestFixture.ts";
requestTestEnvironment();
afterEach(() => vi.restoreAllMocks());
const request = (id: string) =>
  approval({
    id,
    agent: "lead",
    request: { kind: "exec", command: id, cwd: "/", reason: null },
  });
it("reconciles removed requests for A to B to empty", async () => {
  const root = createRoot(
    document.body.appendChild(document.createElement("div")),
  );
  const fixture = createRequestFixture({
    requests: [],
    scope: "replace",
    notify() {},
  });
  fixture.start();
  const render = async (requests: ReturnType<typeof request>[]) =>
    act(async () => {
      fixture.updateRequests(requests);
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            RequestFixture,
            { owner: fixture },
            createElement(Requests, { all: true }),
          ),
        ),
      );
    });
  try {
    await render([request("A")]);
    expect(document.querySelector('[data-request="A"]')).not.toBeNull();
    await render([request("B")]);
    expect(document.querySelectorAll("[data-request]")).toHaveLength(1);
    expect(document.querySelector('[data-request="A"]')).toBeNull();
    expect(document.querySelector('[data-request="B"]')).not.toBeNull();
    await render([]);
    expect(document.querySelectorAll("[data-request]")).toHaveLength(0);
  } finally {
    await act(async () => root.unmount());
    fixture.dispose();
  }
});
it("leaves zero request observers after StrictMode renders twice and disposal", async () => {
  const owners: RequestAreaOwner[] = [];
  const start = RequestAreaOwner.prototype.start;
  vi.spyOn(RequestAreaOwner.prototype, "start").mockImplementation(function (
    this: RequestAreaOwner,
  ) {
    owners.push(this);
    return start.call(this);
  });
  const root = createRoot(
    document.body.appendChild(document.createElement("div")),
  );
  const fixture = createRequestFixture({
    requests: [request("strict")],
    scope: "strict",
    notify() {},
  });
  fixture.start();
  const view = fixture.owner.views.get("strict")!;
  const render = () =>
    act(async () =>
      root.render(
        createElement(
          StrictMode,
          null,
          createElement(
            MantineProvider,
            null,
            createElement(
              RequestFixture,
              { owner: fixture },
              createElement(Requests, { all: true }),
            ),
          ),
        ),
      ),
    );
  await render();
  await render();
  await act(async () => root.unmount());
  fixture.dispose();
  expect(owners).toEqual([fixture.owner]);
  expect(fixture.owner.views.size).toBe(0);
  expect(getObserverTree(view, "draftKey").observers || []).toHaveLength(0);
  for (const owner of owners) {
    const slot = owner.dependencies.store.requests().current;
    if (slot.status === "live" || slot.status === "stale")
      expect(getObserverTree(slot.data.requests).observers || []).toHaveLength(
        0,
      );
    owner.dispose();
  }
});

it("disposes caller-owned request observers after server rendering without a React mount", () => {
  const fixture = createRequestFixture({
    requests: [request("SSR")],
    scope: "ssr",
    notify() {},
  });
  fixture.start();
  enableStaticRendering(true);
  try {
    const html = renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(
          RequestFixture,
          { owner: fixture },
          createElement(Requests, { all: true }),
        ),
      ),
    );
    expect(html).toContain('data-request="SSR"');
    expect(
      getObserverTree(fixture.live.current.requests).observers?.length,
    ).toBeGreaterThan(0);
  } finally {
    fixture.dispose();
    enableStaticRendering(false);
  }
  expect(
    getObserverTree(fixture.live.current.requests).observers || [],
  ).toHaveLength(0);
});
