// @vitest-environment happy-dom
import { act, createElement as h } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import Usage, { createUsageView } from "./Usage.tsx";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import { projectedLimits } from "../../../usage/accountLimits.fixture.ts";
import * as api from "../../../servers/net/api.ts";
import type { PostResult } from "../../../api.ts";
import { ProviderKind, AccountStatus } from "../../../generated/api.ts";
import type { Agent } from "../../../types.ts";

const fixture = vi.hoisted(() => ({
  post: vi.fn(),
  get: vi.fn(async () => ({})),
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => ({ ...api, post: fixture.post, get: fixture.get }),
  }),
);
vi.mock("@mantine/core", async (original) => {
  const actual = await original<typeof import("@mantine/core")>();
  return {
    ...actual,
    Popover: Object.assign(
      (props: Parameters<typeof actual.Popover>[0]) =>
        h(actual.Popover, {
          ...props,
          trapFocus: false,
          returnFocus: false,
          withinPortal: false,
          transitionProps: { duration: 0 },
        }),
      {
        Target: actual.Popover.Target,
        Dropdown: actual.Popover.Dropdown,
        extend: actual.Popover.extend,
      },
    ),
  };
});
vi.mock("../TokenRate/TokenRate.tsx", () => ({ default: () => null }));

it("settles a pending reset while unobserved and permits a retry with its saved identity", async () => {
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
  const agent = { id: "chat", accountKey: "account" } satisfies Agent;
  const models = observableFixture([agent]);
  const limits = projectedLimits("account", 1);
  models.apply([
    {
      collection: "accounts",
      id: "current",
      value: {
        accounts: [
          {
            id: "account",
            label: "Account",
            provider: ProviderKind.codex,
            status: AccountStatus.ready,
            home: "",
            source: "fixture",
          },
        ],
        archivedAccounts: [],
        defaultAccountKey: "account",
        defaultAccountGroups: [],
        supportsDelete: true,
        supportsDisconnect: true,
      },
    },
    {
      collection: "rateLimit",
      id: "account",
      value: {
        ...limits,
        projection: {
          ...limits.projection,
          reset: {
            ...limits.projection.reset,
            present: true,
            accountId: "provider-account",
            credits: [
              {
                id: "credit",
                title: "Credit",
                confirmLabel: "Use credit",
                description: null,
                enabled: true,
                enabledAfter: true,
                expiresAt: null,
                validUntil: null,
              },
            ],
          },
        },
      },
    },
  ]);
  let settle: (value: PostResult<"/api/limits/reset">) => void = () => {};
  const pending = new Promise<PostResult<"/api/limits/reset">>((resolve) => {
    settle = resolve;
  });
  fixture.post.mockImplementation((path: string) =>
    path === "/api/limits/reset" ? pending : Promise.resolve({}),
  );
  let usageStore = createUsageView(
    { ...api, post: fixture.post },
    models.store,
  );
  usageStore.start();
  const host = document.createElement("div");
  document.body.append(host);
  let root = createRoot(host);
  const render = () =>
    root.render(
      h(
        MantineProvider,
        null,
        h(
          models.Provider,
          null,
          h(Usage, { agent, opened: true, onChange: () => {}, usageStore }),
        ),
      ),
    );
  const click = async (label: string) => {
    const button = [
      ...document.querySelectorAll<HTMLButtonElement>("button"),
    ].find((item) => item.textContent === label);
    expect(button).toBeDefined();
    await act(async () => button?.click());
  };
  try {
    await act(async () => render());
    await click("Apply reset");
    await click("Use one reset credit");
    expect(usageStore.resetPending).toBe(true);
    await act(async () => render());
    expect(
      fixture.post.mock.calls.filter(([path]) => path === "/api/limits/reset"),
    ).toHaveLength(1);
    const first = fixture.post.mock.calls.find(
      ([path]) => path === "/api/limits/reset",
    );
    await act(async () => {
      root.unmount();
      usageStore.dispose();
    });
    expect(usageStore.active).toBe(false);
    await act(async () =>
      settle({
        outcome: "uncertain",
        request_id: first?.[1].request_id,
        account_id: "provider-account",
        account_key: "account",
        credit_id: "credit",
        limits: limits.observation,
      }),
    );
    expect(usageStore.resetPending).toBe(false);
    usageStore = createUsageView({ ...api, post: fixture.post }, models.store);
    usageStore.start();
    root = createRoot(host);
    await act(async () => render());
    await click("Apply reset");
    await click("Use one reset credit");
    const attempts = fixture.post.mock.calls.filter(
      ([path]) => path === "/api/limits/reset",
    );
    expect(attempts).toHaveLength(2);
    expect(attempts[1]?.[1].request_id).toBe(first?.[1].request_id);
  } finally {
    await act(async () => root.unmount());
    usageStore.dispose();
    host.remove();
    models.store.dispose();
    fixture.post.mockReset();
    vi.unstubAllGlobals();
  }
});
