import { MantineProvider } from "@mantine/core";
import { NetworkTimeoutError, errorText } from "../../apiErrors.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { createApiScope } from "./api.ts";
import { createNet } from "./net.ts";
import { NetStatus } from "./status.ts";
import { createServerStore } from "../../store/server.ts";
import { createSubscriptions } from "../../sync/subscriptions/module.ts";
import { createMutationJournal } from "../mutationIdentity.ts";
import { NetworkIndicator } from "../../components/servers/NetworkIndicator/NetworkIndicator.tsx";
import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "../../generated/apiSchema.ts";

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    key: (index) => [...values.keys()][index] ?? null,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => {
      values.set(key, value);
    },
    removeItem: (key) => {
      values.delete(key);
    },
    clear: () => values.clear(),
  };
}

afterEach(() => vi.unstubAllGlobals());
it("observes every connection state and finite request outcome without probing", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const pending = Promise.withResolvers<Response>();
  const fetch = vi.fn(() => pending.promise);
  const status = new NetStatus();
  const storage = memoryStorage();
  const api = createApiScope({
    origin: "https://server.test",
    fetch,
    localStorage: storage,
    sessionStorage: memoryStorage(),
    beginMutation: createMutationJournal("fixture", storage),
    beginActivity: () => () => {},
    requestStatus: status,
  });
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: () => () => {},
  });
  const store = createServerStore(subscriptions),
    net = createNet(api, () => store, status);
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(NetworkIndicator, { net }),
        ),
      ),
    );
    for (const [health, expected] of [
      ["live", "live"],
      ["offline", "offline"],
      ["degraded", "reconnecting"],
      ["schema-mismatch", "schema-mismatch"],
      ["live", "schema-mismatch"],
    ] as const) {
      await act(async () => status.receive(health));
      expect(
        host.querySelector(".network-dot")?.getAttribute("data-connection"),
      ).toBe(expected);
    }
    const dot = () => host.querySelector(".network-dot");
    expect(dot()?.textContent).toBe("");
    expect(dot()?.getAttribute("tabindex")).toBe("0");
    expect(dot()?.getAttribute("aria-label")).toBe(
      "Server connection: Update required",
    );
    expect(fetch).not.toHaveBeenCalled();
    let request: Promise<unknown> = Promise.resolve();
    await act(async () => {
      request = api.get("/api/session").catch(() => {});
    });
    expect(dot()?.textContent).toBe("");
    await act(async () => {
      pending.resolve(
        new Response(JSON.stringify({ error: "Request refused" }), {
          status: 400,
          headers: {
            "Content-Type": "application/json",
            [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
          },
        }),
      );
      await request;
    });
    expect(status.inFlight).toBe(0);
    expect(status.lastError).toBe("Request refused");
    expect(dot()?.getAttribute("aria-label")).toBe(
      "Server connection: Update required. Request refused",
    );
    await act(async () => {
      const finish = status.begin();
      finish();
      finish();
    });
    expect(status.inFlight).toBe(0);
  } finally {
    await act(async () => root.unmount());
    store.dispose();
    subscriptions.dispose();
  }
});

function accountingApi(
  fetch: (request: Request) => Promise<Response>,
  unknown: () => Promise<void> = async () => {},
) {
  const status = new NetStatus(),
    completed = vi.fn(),
    cleanup = vi.fn();
  const storage = memoryStorage();
  const journal = createMutationJournal("accounting", storage);
  const beginMutation = Object.assign(
    async () => ({
      requestId: "retained-request",
      finish() {},
      reject() {},
      unknown,
    }),
    { monitorInput: journal.monitorInput },
  );
  const api = createApiScope({
    origin: "https://accounting.test",
    fetch,
    localStorage: storage,
    sessionStorage: memoryStorage(),
    beginMutation,
    beginActivity: () => cleanup,
    requestStatus: {
      begin() {
        const finish = status.begin();
        return (error?: unknown) => {
          completed(error);
          finish(error);
        };
      },
    },
  });
  return { api, status, completed, cleanup };
}

it.each(["get", "post", "access"] as const)(
  "records the caller's normalized deadline error once for %s",
  async (kind) => {
    vi.useFakeTimers();
    try {
      const { api, status, completed, cleanup } = accountingApi(
        (request) =>
          new Promise<Response>((_resolve, reject) => {
            request.signal.addEventListener(
              "abort",
              () => reject(request.signal.reason),
              { once: true },
            );
          }),
      );
      const pending = (
        kind === "get"
          ? api.get("/api/session", { timeoutMs: 25 })
          : kind === "post"
            ? api.post("/api/organization", { id: "chat" }, { timeoutMs: 25 })
            : api.serverAccess(
                "POST",
                { action: "discover", requestId: "same-request" },
                25,
              )
      ).catch((error: unknown) => error);
      await vi.advanceTimersByTimeAsync(25);
      const error = await pending;
      expect(error).toBeInstanceOf(NetworkTimeoutError);
      expect(status.lastError).toBe(errorText(error));
      expect(completed).toHaveBeenCalledExactlyOnceWith(error);
      expect(status.inFlight).toBe(0);
      if (kind === "post") expect(cleanup).toHaveBeenCalledOnce();
    } finally {
      vi.useRealTimers();
    }
  },
);

it.each(["settles", "fails"] as const)(
  "keeps a lost POST in flight until its uncertainty journal %s and cleanup completes",
  async (outcome) => {
    const entered = Promise.withResolvers<void>(),
      journal = Promise.withResolvers<void>();
    const lost = new TypeError("Response lost");
    const journalError = new Error(
      "The uncertainty journal could not be saved",
    );
    const { api, status, completed, cleanup } = accountingApi(
      async () => {
        throw lost;
      },
      () => {
        entered.resolve();
        return journal.promise;
      },
    );
    const pending = api
      .post("/api/organization", { id: "chat" })
      .catch((error: unknown) => error);
    try {
      await entered.promise;
      expect(status.inFlight).toBe(1);
      expect(status.lastError).toBeNull();
      expect(completed).not.toHaveBeenCalled();
      expect(cleanup).not.toHaveBeenCalled();
    } finally {
      if (outcome === "fails") journal.reject(journalError);
      else journal.resolve();
      await pending;
    }
    const error = outcome === "fails" ? journalError : lost;
    expect(await pending).toBe(error);
    expect(status.inFlight).toBe(0);
    expect(completed).toHaveBeenCalledExactlyOnceWith(error);
    expect(status.lastError).toBe(errorText(error));
    expect(cleanup.mock.invocationCallOrder[0]).toBeLessThan(
      completed.mock.invocationCallOrder[0] ?? 0,
    );
  },
);
