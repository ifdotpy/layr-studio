import { createBackgroundTaskArea } from "../support/backgroundTaskActions.tsx";
import { initSync } from "../../../../wasm/generated/client";
// @vitest-environment happy-dom
import {
  ProcessSourceKind,
  BackgroundTaskKind,
  BackgroundTaskKindLabel,
  BackgroundTaskIndicator,
  BackgroundTaskLabel,
  ProcessStdinState,
} from "../../../../generated/api.ts";
import { TaskStatus as GeneratedApiTaskStatus } from "../../../../generated/api.ts";
import { webcrypto } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { createSubscriptions } from "../../../../sync/subscriptions/module.ts";
import { createServerStore } from "../../../../store/server.ts";
import { ServerStoreProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { agentSummaryFixture } from "../../../../servers/agentSummaryFixture.ts";
import {
  BackgroundTaskActionsProvider,
  type BackgroundTaskActions,
} from "../support/backgroundTaskActions.tsx";
import { afterEach, expect, it, vi } from "vitest";
import { createApiScope, type ServerApi } from "../../../../servers/net/api.ts";
import { createMutationJournal } from "../../../../servers/mutationIdentity.ts";
import { monitorInputStoreFixture } from "../../../../servers/monitorInputStore.fixture.ts";

const hooks = vi.hoisted(() => ({ api: undefined as ServerApi | undefined }));
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => hooks.api,
    useServerServices: () => null,
  }),
);
import { ProcessInput } from "../ProcessInput/ProcessInput.tsx";

function memoryStorage(): Storage {
  const rows = new Map<string, string>();
  return {
    get length() {
      return rows.size;
    },
    key: (index) => [...rows.keys()][index] ?? null,
    getItem: (key) => rows.get(key) ?? null,
    setItem: (key, value) => {
      rows.set(key, value);
    },
    removeItem: (key) => {
      rows.delete(key);
    },
    clear: () => rows.clear(),
  };
}

async function fixture() {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  // LAN HTTP has secure randomness but no SubtleCrypto.
  vi.stubGlobal("crypto", {
    getRandomValues: webcrypto.getRandomValues.bind(webcrypto),
  });
  initSync({
    module: new Uint8Array(
      readFileSync(
        join(import.meta.dirname, "../../../../wasm/generated/client_bg.wasm"),
      ),
    ),
  });
  const storage = memoryStorage();
  const journal = monitorInputStoreFixture();
  const writes = new Map<string, string>();
  const requests: Request[] = [];
  let lost = true;
  let receiptStatus = "acknowledged";
  let receiptOverride: unknown;
  let receiptWait: Promise<void> | undefined;
  let receiptOnly = false;
  const fetch = async (request: Request) => {
    requests.push(request);
    if (request.method === "GET") {
      await receiptWait;
      if (receiptOverride) return Response.json(receiptOverride);
      const id = new URL(request.url).searchParams.get("requestId");
      return Response.json({
        found: true,
        result: {
          requestId: id,
          status:
            receiptStatus === "acknowledged" || receiptStatus === "rejected"
              ? "completed"
              : "uncertain",
          response: {
            kind: "monitor_input",
            monitorId: "monitor-one",
            status: receiptStatus,
          },
        },
      });
    }
    const id = request.headers.get("X-Studio-Request-Id")!;
    const body = await request.text();
    if (writes.has(id)) expect(writes.get(id)).toBe(body);
    else writes.set(id, body);
    if (lost) {
      lost = false;
      throw new Error("HTTP success response was lost");
    }
    return Response.json({ requestId: id, status: "acknowledged" });
  };
  const open = () =>
    createApiScope({
      origin: "http://192.0.2.10:49171",
      fetch,
      localStorage: storage,
      sessionStorage: storage,
      beginMutation: createMutationJournal(null, storage, journal.store(null)),
    });
  hooks.api = open();
  const task: import("../support/backgroundTaskModel.ts").DisplayProcessTask = {
    id: "monitor-one",
    agent: "owner",
    created: 1,
    status: GeneratedApiTaskStatus.running,
    key: "monitor-key",
    generation: "generation",
    accountKey: "target",
    epoch: 3,
    name: "Command",
    source: ProcessSourceKind.monitor,
    bytes: 0,
    cancelRequested: false,
    outputTruncated: false,
    outputPatterns: [],
    tail: "",
    display: {
      kind: BackgroundTaskKind.monitor,
      kindLabel: BackgroundTaskKindLabel.monitor,
      title: "Command",
      active: true,
      indicator: BackgroundTaskIndicator.working,
      label: BackgroundTaskLabel.running,
      inBackgroundList: true,
      actions: ["stop", "input", "resize", "readOutput"],
      stdinState: ProcessStdinState.open,
    },
  } as const;
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: (accept) => {
      accept("live", "open", {});
      return () => {};
    },
  });
  const store = createServerStore(subscriptions);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const notify = vi.fn();
  const initialPost = vi.spyOn(hooks.api!, "post");
  let area: BackgroundTaskActions = createBackgroundTaskArea(
    hooks.api!,
    store,
    "local",
    {
      opened: true,
      leadId: "owner",
      close() {},
      openAgent() {},
      notify,
    },
  );
  function Area() {
    return createElement(
      BackgroundTaskActionsProvider,
      { value: area },
      createElement(ProcessInput, {
        task: receiptOnly
          ? { ...task, display: { ...task.display, actions: ["readOutput"] } }
          : task,
      }),
    );
  }
  let generation = 0;
  const render = () =>
    act(async () =>
      root.render(
        createElement(
          ServerStoreProvider,
          { value: store },
          createElement(
            MantineProvider,
            { env: "test" },
            createElement(Area, { key: generation }),
          ),
        ),
      ),
    );
  await render();
  await act(async () => new Promise((resolve) => setTimeout(resolve, 100)));
  await act(async () => {
    subscriptions.publish({ kind: "state" }, [
      {
        collection: "agentSummary",
        id: "owner",
        value: agentSummaryFixture("owner", { accountKey: "target", epoch: 3 }),
      },
    ]);
    await Promise.resolve();
  });
  const input = () =>
    host.querySelector<HTMLInputElement>("input[aria-label='Terminal input']")!;
  const type = async (text: string) =>
    act(async () => {
      const field = input();
      Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )!.set!.call(field, text);
      field.dispatchEvent(new Event("input", { bubbles: true }));
    });
  const settle = async () =>
    act(async () => {
      await vi.waitFor(() => expect(area.pending).toBe(false));
    });
  const send = async () => {
    await act(async () =>
      host
        .querySelector("form")!
        .dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        ),
    );
    await settle();
  };
  const check = async () => {
    await act(async () =>
      [...host.querySelectorAll("button")]
        .find((button) => button.textContent === "Check input receipt")!
        .click(),
    );
  };
  cleanup = async () => {
    await act(async () => root.unmount());
    store.dispose();
    subscriptions.dispose();
    host.remove();
  };
  return {
    storage,
    pauseReceipt: () => {
      let accept = () => {};
      receiptWait = new Promise<void>((resolve) => {
        accept = resolve;
      });
      return () => {
        receiptWait = undefined;
        accept();
      };
    },
    initialPost,
    switchScope: async (leadId = "next-owner") => {
      const api = open(),
        post = vi.spyOn(api, "post");
      area = createBackgroundTaskArea(api, store, "local", {
        opened: true,
        leadId,
        close() {},
        openAgent() {},
        notify,
      });
      await render();
      return post;
    },
    notify,
    writes,
    requests,
    open,
    reload: async () => {
      hooks.api = open();
      generation++;
      await render();
    },
    type,
    input,
    render,
    status: () => host.querySelector('[role="status"]')?.textContent,
    send,
    check,
    setReceipt: (status: string) => {
      receiptStatus = status;
    },
    metadata: () => journal.read(null, "monitor-one"),
    setReceiptObject: (value: unknown) => {
      receiptOverride = value;
    },
    receiptOnly: () => {
      receiptOnly = true;
    },
    hasForm: () => !!host.querySelector("form"),
  };
}

let cleanup: (() => Promise<void>) | undefined;
afterEach(async () => {
  await cleanup?.();
  cleanup = undefined;
  vi.unstubAllGlobals();
});

it("the real Send line handler retries a lost success with the same ID and permits a fresh deliberate identical line", async () => {
  const f = await fixture();
  await f.type("private stdin");
  await f.send();
  expect(f.input().value).toBe("private stdin");
  expect(f.writes.size, JSON.stringify(f.notify.mock.calls)).toBe(1);
  expect(f.storage.length).toBe(0);
  expect(JSON.stringify(f.metadata())).not.toContain("private stdin");
  await f.type("different input");
  await f.send();
  expect(f.requests).toHaveLength(1);
  await f.type("private stdin");
  await f.send();
  expect(f.requests[1]?.headers.get("X-Studio-Request-Id")).toBe(
    f.requests[0]?.headers.get("X-Studio-Request-Id"),
  );
  expect(f.writes.size).toBe(1);
  expect(f.input().value).toBe("");
  await f.type("private stdin");
  await f.send();
  expect(f.writes.size).toBe(2);
  expect(f.requests[2]?.headers.get("X-Studio-Request-Id")).not.toBe(
    f.requests[0]?.headers.get("X-Studio-Request-Id"),
  );
});

it("the real receipt button recovers after reload without the secret and never clears an unknown result", async () => {
  const f = await fixture();
  await f.type("private stdin");
  await f.send();
  await f.reload();
  expect(f.input().value).toBe("");
  f.receiptOnly();
  await f.render();
  expect(f.hasForm()).toBe(false);
  f.setReceipt("unknown");
  await f.check();
  await vi.waitFor(() =>
    expect(f.status()).toBe(
      "The input result is unknown. Do not send different input.",
    ),
  );
  expect(await hooks.api!.pendingMonitorInput("monitor-one")).not.toBeNull();
  f.setReceipt("acknowledged");
  await f.check();
  await vi.waitFor(() =>
    expect(f.status()).toBe("The process acknowledged the input."),
  );
  expect(await hooks.api!.pendingMonitorInput("monitor-one")).toBeNull();
  expect(f.writes.size).toBe(1);
  const checks = f.requests.filter((request) => request.method === "GET");
  expect(checks).toHaveLength(2);
  expect(new URL(checks[0]!.url).searchParams.get("accountKey")).toBe("target");
  expect(new URL(checks[0]!.url).searchParams.get("agent")).toBe("owner");
  expect(new URL(checks[0]!.url).searchParams.get("epoch")).toBe("3");
  expect(checks.every((request) => !request.url.includes("private"))).toBe(
    true,
  );
});

it("the receipt reader keeps a pending identity for missing or mismatched evidence and releases an exact rejection", async () => {
  const f = await fixture();
  await f.type("private stdin");
  await f.send();
  const pending = await hooks.api!.pendingMonitorInput("monitor-one");
  const requestId = pending!.requestId;
  const exact = {
    requestId,
    status: "completed",
    response: {
      kind: "monitor_input",
      monitorId: "monitor-one",
      status: "acknowledged",
    },
  };
  for (const receipt of [
    { found: false, result: null },
    { found: true, result: { ...exact, requestId: "another-request" } },
    { found: true, result: { ...exact, status: "uncertain" } },
    {
      found: true,
      result: { ...exact, response: { ...exact.response, kind: "stop" } },
    },
    {
      found: true,
      result: {
        ...exact,
        response: { ...exact.response, monitorId: "another-monitor" },
      },
    },
    {
      found: true,
      result: { ...exact, response: { ...exact.response, status: "pending" } },
    },
  ]) {
    f.setReceiptObject(receipt);
    await f.check();
    await act(async () => {
      await vi.waitFor(() =>
        expect(f.status()).toBe(
          "The input result is unknown. Do not send different input.",
        ),
      );
    });
    expect(await hooks.api!.pendingMonitorInput("monitor-one")).toEqual(
      pending,
    );
  }
  f.setReceiptObject({
    found: true,
    result: { ...exact, response: { ...exact.response, status: "rejected" } },
  });
  await f.check();
  await act(async () => {
    await vi.waitFor(() =>
      expect(f.status()).toBe("The process rejected the input."),
    );
  });
  expect(await hooks.api!.pendingMonitorInput("monitor-one")).toBeNull();
  expect(f.writes.size).toBe(1);
});

it("uses the new activity scope for input after its provider changes on the same element", async () => {
  const f = await fixture();
  await f.type("old scope draft");
  const post = await f.switchScope();
  expect(f.input().value).toBe("");
  await f.type("new scope input");
  const sameScopePost = await f.switchScope();
  expect(f.input().value).toBe("new scope input");
  await f.send();
  expect(sameScopePost).toHaveBeenCalledOnce();
  expect(post).not.toHaveBeenCalled();
  expect(f.initialPost).not.toHaveBeenCalled();
});

it("keeps input disabled while a receipt check is pending", async () => {
  const f = await fixture();
  await f.type("receipt witness");
  await f.send();
  const resume = f.pauseReceipt();
  try {
    await f.check();
    expect(f.input().disabled).toBe(true);
  } finally {
    await act(async () => resume());
  }
  expect(f.input().disabled).toBe(false);
});
