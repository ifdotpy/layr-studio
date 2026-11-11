// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createApiScope } from "../servers/net/api.ts";
import { createMutationJournal } from "../servers/mutationIdentity.ts";
import { preferenceMemory } from "../servers/storage.ts";
import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "../generated/apiSchema.ts";
import { FrontendLocality } from "../generated/api.ts";
import { useLimitRefresh } from "./useLimitRefresh.ts";

const stops: (() => void)[] = [];
beforeEach(() => vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true));
afterEach(async () => {
  await act(async () => stops.splice(0).forEach((stop) => stop()));
  preferenceMemory.clear();
  vi.unstubAllGlobals();
});
async function fixture(label = "First", token = "credential-one") {
  const requests: { request: Request; finish(response: Response): void }[] = [];
  const api = createApiScope({
    origin: "https://server.test",
    fetch: (request) =>
      new Promise<Response>((finish) =>
        requests.push({
          request: new Request(request, {
            headers: {
              ...Object.fromEntries(request.headers),
              "X-Fixture-Credential": token,
            },
          }),
          finish,
        }),
      ),
    localStorage: preferenceMemory,
    sessionStorage: preferenceMemory,
    beginMutation: createMutationJournal(label, preferenceMemory),
  });
  api.receiveStreamSession({
    routes: [],
    locality: FrontendLocality.local,
    cachePolicies: [],
    sessionId: label,
    workspaceId: "a".repeat(32),
    token,
  });
  let current!: ReturnType<typeof useLimitRefresh>;
  function Probe() {
    current = useLimitRefresh(api.get);
    return null;
  }
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  await act(async () => root.render(createElement(Probe)));
  stops.push(() => {
    root.unmount();
    host.remove();
  });
  return {
    get current() {
      return current;
    },
    requests,
    finish(index: number) {
      requests[index]!.finish(
        Response.json(
          { accountKey: "same", data: null },
          { headers: { [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH } },
        ),
      );
    },
  };
}
test("cached and forced attempts keep loading until both finish", async () => {
  const f = await fixture();
  let a!: Promise<void>, b!: Promise<void>;
  await act(async () => {
    a = f.current.reload("same");
    expect(f.current.reload("same")).toBe(a);
    b = f.current.reload("same", true);
  });
  expect(f.requests).toHaveLength(2);
  expect(f.current.isRefreshing("same")).toBe(true);
  await act(async () => {
    f.finish(1);
    await b;
  });
  expect(f.current.isRefreshing("same")).toBe(true);
  await act(async () => {
    f.finish(0);
    await a;
  });
  expect(f.current.isRefreshing("same")).toBe(false);
});
test.each([
  [0, 1],
  [1, 0],
])(
  "two forced refreshes settle independently, request %i first",
  async (first, last) => {
    const f = await fixture();
    let requests!: Promise<void>[];
    await act(async () => {
      requests = [
        f.current.reload("same", true),
        f.current.reload("same", true),
      ];
    });
    await act(async () => {
      f.finish(first);
      await requests[first];
    });
    expect(f.current.isRefreshing("same")).toBe(true);
    await act(async () => {
      f.finish(last);
      await requests[last];
    });
    expect(f.current.isRefreshing("same")).toBe(false);
  },
);
test("the same account id on two credential scopes never shares a pending request", async () => {
  const first = await fixture("First", "fixture-one"),
    second = await fixture("Second", "fixture-two");
  let a!: Promise<void>, b!: Promise<void>;
  await act(async () => {
    a = first.current.reload("same", true);
    b = second.current.reload("same", true);
  });
  expect(first.requests[0]?.request.headers.get("X-Fixture-Credential")).toBe(
    "fixture-one",
  );
  expect(second.requests[0]?.request.headers.get("X-Fixture-Credential")).toBe(
    "fixture-two",
  );
  await act(async () => {
    first.finish(0);
    await a;
  });
  expect(first.current.isRefreshing("same")).toBe(false);
  expect(second.current.isRefreshing("same")).toBe(true);
  await act(async () => {
    second.finish(0);
    await b;
  });
  expect(second.current.isRefreshing("same")).toBe(false);
});
