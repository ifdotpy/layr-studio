// @vitest-environment happy-dom
import type { PostResult } from "../api";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { useClaudeLoginProgress } from "./useClaudeLoginProgress";
import type { ResourceVersion } from "../sync/subscriptions/events";

const service = vi.hoisted(() => ({
  get: vi.fn(),
  resources: {
    watchResourceChanges: vi.fn(),
    watchResourceConnection: vi.fn(),
  },
}));
vi.mock("../servers/ServerServicesContext", () => ({
  useServerApi: () => ({ get: service.get }),
  useServerResources: () => service.resources,
}));

it("lets stream progress win over an older recovery read and releases its subscription", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  let changed: (version?: ResourceVersion) => void = () => {};
  const stopChanges = vi.fn(),
    stopConnection = vi.fn();
  service.resources.watchResourceChanges.mockImplementation(
    (_resource, accept) => {
      changed = accept;
      return stopChanges;
    },
  );
  service.resources.watchResourceConnection.mockReturnValue(stopConnection);
  let finish: (
    receipt: PostResult<"/api/accounts/claude/login">,
  ) => void = () => {};
  const get = service.get;
  get.mockReset().mockImplementation(
    () =>
      new Promise<PostResult<"/api/accounts/claude/login">>((resolve) => {
        finish = resolve;
      }),
  );
  const apply = vi.fn(),
    failed = vi.fn();
  const model = {
    current: {
      locked: false,
      requestId: "login",
      readGeneration: 0,
      refresh: () => {},
    },
  };
  const root = createRoot(document.createElement("div"));
  function Caller() {
    useClaudeLoginProgress(model, true, { apply, failed });
    return null;
  }
  try {
    await act(async () => root.render(createElement(Caller)));
    expect(get).not.toHaveBeenCalled();
    model.current.refresh();
    expect(get).toHaveBeenCalledOnce();
    changed({
      epoch: "one",
      revision: 1,
      progress: {
        resource: { kind: "auth-progress", requestId: "login" },
        value: { requestId: "login", accountKey: "account", status: "ready" },
      },
    });
    finish({ requestId: "login", accountKey: "account", status: "pending" });
    await act(async () => {});
    expect(apply).toHaveBeenCalledTimes(1);
    expect(apply).toHaveBeenLastCalledWith({
      requestId: "login",
      accountKey: "account",
      status: "ready",
    });
    model.current.locked = true;
    changed({
      epoch: "one",
      revision: 1,
      progress: {
        resource: { kind: "auth-progress", requestId: "login" },
        value: { requestId: "login", accountKey: "account", status: "pending" },
      },
    });
    expect(apply).toHaveBeenCalledTimes(1);
  } finally {
    await act(async () => root.unmount());
    vi.unstubAllGlobals();
  }
  expect(stopChanges).toHaveBeenCalledOnce();
  expect(stopConnection).toHaveBeenCalledOnce();
  model.current.refresh();
  expect(get).toHaveBeenCalledOnce();
  expect(failed).not.toHaveBeenCalled();
});
