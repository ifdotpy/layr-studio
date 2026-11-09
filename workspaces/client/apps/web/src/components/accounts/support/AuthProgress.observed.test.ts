import { ProviderKind } from "../../../generated/api.ts";
// @vitest-environment happy-dom
import { act, createElement, type ComponentProps, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { transientTestFixture } from "../../../sync/subscriptions/transientTestFixture.ts";
import type { ResourceProgress } from "../../../sync/subscriptions/contracts.ts";

const fixture = vi.hoisted(() => ({
  services: undefined as
    | (Omit<ReturnType<typeof transientTestFixture>, "subscriptions"> & {
        subscriptions: ReturnType<
          typeof import("../../../sync/subscriptions/module.ts").createSubscriptions
        >;
      })
    | undefined,
  post: vi.fn(),
  get: vi.fn(),
  retry: vi.fn(),
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerApi: () => ({
      get: fixture.get,
      post: fixture.post,
      saved: (key: string, fallback: unknown) =>
        key.endsWith("code-submission-uncertain") ? fallback : "login",
      save() {},
      stableRequestId: () => "http-request",
      clearStableRequestId() {},
    }),
    useServerStore: () => fixture.services!.store,
    useServerResources: () => ({
      ...fixture.services!.subscriptions,
      retryNow: fixture.retry,
    }),
  }),
);
import AccountSignIn from "../AccountSignIn/AccountSignIn.tsx";
import ClaudeSignIn from "../ClaudeSignIn/ClaudeSignIn.tsx";

async function mount(child: ReactNode) {
  vi.useFakeTimers();
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
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
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  await act(async () =>
    root.render(createElement(MantineProvider, null, child)),
  );
  await act(async () => vi.advanceTimersByTimeAsync(100));
  return {
    async close() {
      await act(async () => root.unmount());
      host.remove();
      fixture.services!.close();
      vi.useRealTimers();
      vi.unstubAllGlobals();
      vi.clearAllMocks();
    },
  };
}
async function deliver(
  value: Extract<
    ResourceProgress,
    { resource: { kind: "auth-progress" } }
  >["value"],
  revision: number,
) {
  const ref = { kind: "auth-progress", requestId: "login" } as const;
  await act(async () => {
    fixture.services!.emit(ref, {
      epoch: "one",
      revision,
      progress: { resource: ref, value },
    });
    await vi.advanceTimersByTimeAsync(0);
  });
}
function button(label: string) {
  return [...document.querySelectorAll<HTMLButtonElement>("button")].find(
    (button) => button.textContent?.trim() === label,
  )!;
}

it("pending to uncertain allows a user retry with the original login identity and observation sends no commands", async () => {
  fixture.services = transientTestFixture();
  const pending = {
    requestId: "login",
    accountKey: "account",
    status: "pending",
  } as const;
  const state = {
    scope: "local",
    data: {
      accounts: [],
      archivedAccounts: [],
      defaultAccountKey: "default",
      defaultAccountGroups: [],
      logins: [pending],
      supportsDelete: true,
      supportsDisconnect: true,
    },
    setData: vi.fn(),
    error: "",
    refresh: vi.fn(async () => null),
  } satisfies ComponentProps<typeof AccountSignIn>["state"];
  fixture.post.mockResolvedValue(pending);
  const f = await mount(createElement(AccountSignIn, { state }));
  try {
    expect(button("Sign in").disabled).toBe(true);
    await deliver({ ...pending, status: "uncertain" }, 1);
    expect(button("Sign in").disabled).toBe(false);
    await act(async () => {
      fixture.services!.connection("offline");
      fixture.services!.connection("live");
    });
    await deliver({ ...pending, status: "uncertain" }, 2);
    expect(fixture.post).not.toHaveBeenCalled();
    expect(fixture.get).not.toHaveBeenCalled();
    expect(state.refresh).not.toHaveBeenCalled();
    await act(async () => button("Sign in").click());
    expect(fixture.post).toHaveBeenCalledOnce();
    expect(fixture.post.mock.calls[0]?.[1]).toMatchObject({
      login_id: "login",
    });
  } finally {
    await f.close();
  }
});

it("Claude completion invokes its reconnect owner exactly once across replay and reconnect", async () => {
  fixture.services = transientTestFixture();
  fixture.post.mockReset();
  const reconnect = vi.fn(async () => {});
  const account = {
    id: "account",
    label: "Claude",
    provider: ProviderKind.claude,
  } as ComponentProps<typeof ClaudeSignIn>["account"];
  const f = await mount(
    createElement(ClaudeSignIn, {
      account,
      scope: "fixture",
      onClose: vi.fn(),
      onReady: reconnect,
    }),
  );
  const ready = {
    requestId: "login",
    accountKey: "account",
    status: "ready",
  } as const;
  try {
    await deliver({ ...ready, status: "pending" }, 1);
    expect(reconnect).not.toHaveBeenCalled();
    await deliver(ready, 2);
    expect(reconnect).toHaveBeenCalledOnce();
    await act(async () => {
      fixture.services!.connection("offline");
      fixture.services!.connection("live");
    });
    await deliver(ready, 3);
    expect(reconnect).toHaveBeenCalledOnce();
    expect(fixture.post).not.toHaveBeenCalled();
    expect(fixture.get).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain("Signed in");
  } finally {
    await f.close();
  }
});

it.each(["ready", "codeSubmitted"] as const)(
  "late recovery cannot regress a command's %s receipt across reconnect and replay",
  async (completed) => {
    type Receipt =
      import("../../../api.ts").PostResult<"/api/accounts/claude/login">;
    let finish!: (value: Receipt) => void;
    const recover = vi.fn(
      () =>
        new Promise<Receipt>((resolve) => {
          finish = resolve;
        }),
    );
    fixture.services = transientTestFixture({
      authProgress: recover,
      voice: vi.fn(),
    });
    fixture.post.mockReset();
    const reconnect = vi.fn(async () => {});
    const account = {
      id: "account",
      label: "Claude",
      provider: ProviderKind.claude,
    } as ComponentProps<typeof ClaudeSignIn>["account"];
    const result: Receipt = {
      requestId: "login",
      accountKey: "account",
      status: completed === "ready" ? "ready" : "pending",
      codeSubmitted: true,
    };
    fixture.post.mockResolvedValue(result);
    const f = await mount(
      createElement(ClaudeSignIn, {
        account,
        scope: "fixture",
        onClose: vi.fn(),
        onReady: reconnect,
      }),
    );
    try {
      await act(async () => {
        fixture.services!.connection("offline");
        fixture.services!.connection("live");
      });
      await deliver(null, 1);
      expect(recover).toHaveBeenCalledOnce();
      await act(async () => button("Retry sign-in").click());
      expect(fixture.post).toHaveBeenCalledOnce();
      expect(document.body.textContent).not.toContain(
        "Submit authorization response",
      );
      await act(async () => {
        finish({ ...result, status: "pending", codeSubmitted: false });
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(document.body.textContent).not.toContain(
        "Submit authorization response",
      );
      if (completed === "ready")
        expect(document.body.textContent).toContain("Signed in");
      else
        expect(document.body.textContent).toContain(
          "Waiting for Claude to verify",
        );
      await deliver({ ...result, status: "ready" }, 2);
      await act(async () => {
        fixture.services!.connection("offline");
        fixture.services!.connection("live");
      });
      await deliver({ ...result, status: "ready" }, 3);
      expect(reconnect).toHaveBeenCalledOnce();
      expect(fixture.post).toHaveBeenCalledOnce();
    } finally {
      await f.close();
    }
  },
);

it("restores a sign-in code after a resource hint finishes behind inline progress", async () => {
  const { createResourceEvents } =
    await import("../../../sync/resourceEvents.ts");
  const { createSubscriptions } =
    await import("../../../sync/subscriptions/module.ts");
  const { createDataObserver } =
    await import("../../../sync/subscriptions/data.ts");
  const { createServerStore } = await import("../../../store/server.ts");
  const { API_SCHEMA_HASH } = await import("../../../generated/apiSchema.ts");
  const workspaceId = "b".repeat(32);
  const source = Object.assign(new EventTarget(), {
    onopen: null as ((event: Event) => void) | null,
    onerror: null as ((event: Event) => void) | null,
    close: vi.fn(),
  });
  const emit = (name: string, value: unknown) =>
    source.dispatchEvent(
      new MessageEvent(name, { data: JSON.stringify(value) }),
    );
  const api = {
    get: fixture.get,
    post: fixture.post,
    clearSchemaUpdateAttemptAfterMatch: vi.fn(),
    isApiSchemaMismatch: () => false,
    matchingApiSchemaResponseGeneration: () => 0,
    markApiSchemaMismatch: vi.fn(),
    onApiSchemaMismatch: () => () => {},
  };
  const transport = createResourceEvents({
    api,
    sync: {
      syncDatabase: async () => ({ workspaceId }),
      persistResourceEntityChanges: async () => {},
    },
    tokenRates: {
      configureTokenRateStream() {},
      receiveResourceTokenRates() {
        return true;
      },
      getWorkspaceTokenRates: () => ({ records: [] }),
    },
    openEvents: () => source,
  });
  const subscriptions = createSubscriptions(
    transport,
    createDataObserver(
      () => ({ subscribeTranscriptProjection: () => () => {} }),
      transport,
      undefined,
      api,
    ),
  );
  const store = createServerStore(subscriptions);
  const old = transientTestFixture();
  fixture.services = {
    ...old,
    store,
    subscriptions,
    close() {
      store.dispose();
      subscriptions.dispose();
      transport.dispose();
      old.close();
    },
  };
  const account = {
    id: "default",
    label: "Default",
    home: "/fixture",
    provider: ProviderKind.codex,
    source: "Codex",
    status: "ready",
    eligibility: { kind: "available" },
  } as const;
  const state = {
    scope: "local",
    data: {
      accounts: [account],
      archivedAccounts: [],
      defaultAccountKey: "default",
      defaultAccountGroups: [],
      logins: [],
      supportsDelete: true,
      supportsDisconnect: true,
    },
    setData: vi.fn(),
    error: "",
    refresh: vi.fn(async () => null),
  } satisfies ComponentProps<typeof AccountSignIn>["state"];
  const f = await mount(
    createElement(AccountSignIn, { state, targetAccount: account }),
  );
  const ref = { kind: "auth-progress", requestId: "login" } as const;
  try {
    await act(async () => {
      source.onopen?.(new Event("open"));
      emit("session", { sessionId: "session", workspaceId, cachePolicies: [] });
      emit("api-schema", { hash: API_SCHEMA_HASH });
      emit("resources", {
        protocol: 4,
        workspaceId,
        epoch: "one",
        revision: 6,
        reason: "initial",
        resources: [ref],
        resourceVersions: [{ resource: ref, revision: 1 }],
      });
      emit("resource-progress", {
        protocol: 4,
        workspaceId,
        epoch: "one",
        revision: 6,
        resource: ref,
        value: {
          requestId: "login",
          accountKey: "default",
          reauthAccountKey: "default",
          status: "pending",
          userCode: "ABCD-1234",
          verificationUrl: "https://auth.openai.com/codex/device",
        },
      });
      await vi.advanceTimersByTimeAsync(0);
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(document.body.textContent).toContain("ABCD-1234");
    expect(fixture.post).not.toHaveBeenCalled();
  } finally {
    await f.close();
  }
});
