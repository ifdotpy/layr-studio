// @vitest-environment happy-dom
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createServerStore } from "../../../store/server.ts";
import { createSubscriptions } from "../../../sync/subscriptions/module.ts";
import {
  AccountStatus as GeneratedApiAccountStatus,
  ProviderKind,
} from "../../../generated/api.ts";
import { act, createElement, useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test, vi } from "vitest";
import { projectedLimits } from "../../../usage/accountLimits.fixture.ts";
import type { AccountLimitsSnapshot } from "../../../usage/accountUsage.ts";
import type { Account } from "../support/Accounts.tsx";
import { accountLimitView } from "../support/AccountTiles.tsx";
import { AccountTileLabel } from "../AccountTileLabel/AccountTileLabel.tsx";
import { AccountTiles } from "./AccountTiles.tsx";

const checkboxProps = vi.hoisted(() => vi.fn());
const tooltipProps = vi.hoisted(() => vi.fn());
vi.mock("@mantine/core", async (load) => {
  const actual = await load<typeof import("@mantine/core")>();
  return {
    ...actual,
    Checkbox: (props: Parameters<typeof actual.Checkbox>[0]) => {
      checkboxProps(props);
      return createElement("input", {
        type: "checkbox",
        "aria-label": props["aria-label"],
      });
    },
    Tooltip: (props: {
      label: string;
      children: React.ReactNode;
      events?: object;
    }) => {
      tooltipProps(props);
      return createElement("span", { title: props.label }, props.children);
    },
  };
});

test("account tiles show an allow checkbox for workers", () => {
  checkboxProps.mockClear();
  tooltipProps.mockClear();
  const onExcludedChange = vi.fn();
  const accounts: Account[] = [
    {
      id: "lead",
      label: "Lead",
      home: "",
      source: "fixture",
      provider: ProviderKind.codex,
      status: GeneratedApiAccountStatus.ready,
    },
    {
      id: "worker",
      label: "Worker",
      home: "",
      source: "fixture",
      provider: ProviderKind.codex,
      status: GeneratedApiAccountStatus.ready,
    },
  ];
  const html = renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(
        ServerStoreProvider,
        {
          value: createServerStore(
            createSubscriptions({
              watchResourceChanges: () => () => {},
              watchResourceConnection: () => () => {},
            }),
          ),
        },
        createElement(AccountTiles, {
          label: "Chat accounts",
          accounts,
          value: "lead",
          onChange: () => {},
          excludedKeys: ["lead"],
          onExcludedKeysChange: onExcludedChange,
          exclusionRole: "worker",
        }),
      ),
    ),
  );
  expect(html).toContain('aria-label="Use Lead for workers"');
  expect(html).toContain('aria-label="Select account Lead"');
  expect(html).toContain('aria-label="Use Worker for workers"');
  expect(html).not.toContain("Exclude for workers and reviewers");
  expect(checkboxProps.mock.calls[0]![0].label).toBeUndefined();
  expect(checkboxProps.mock.calls[0]![0].checked).toBe(false);
  expect(checkboxProps.mock.calls[1]![0].checked).toBe(true);
  expect(checkboxProps.mock.calls[1]![0].disabled).toBe(true);
  expect(html).toContain('title="Use for workers"');
  expect(tooltipProps).toHaveBeenCalledWith(
    expect.objectContaining({
      label: "Use for workers",
      events: { hover: true, focus: true, touch: false },
    }),
  );
  expect(html.indexOf('aria-label="Use Lead')).toBeLessThan(
    html.indexOf('aria-label="Select account Lead"'),
  );
  checkboxProps.mock.calls[0]![0].onChange({
    currentTarget: { checked: true },
  });
  expect(onExcludedChange).toHaveBeenCalledWith([]);
  checkboxProps.mock.calls[1]![0].onChange({
    currentTarget: { checked: false },
  });
  expect(onExcludedChange).toHaveBeenCalledWith(["lead", "worker"]);
});

const reading = (fields: object = {}) =>
  ({
    accountKey: "work",
    at: 100,
    data: { accountId: null, rateLimits: { primary: { usedPercent: 67 } } },
    error: null,
    ...fields,
  }) as unknown as AccountLimitsSnapshot;
const account = (status: Account["status"] = "ready") => ({
  id: "work",
  accountId: null,
  status,
});

test("an account picker tile states the remaining limit next to the name", () => {
  const markup = renderToStaticMarkup(
    createElement(AccountTileLabel, { name: "Work", remaining: 33 }),
  );
  expect(markup).toContain("Work");
  expect(markup).toContain("33% left");
  expect(
    renderToStaticMarkup(
      createElement(AccountTileLabel, { name: "Work", remaining: null }),
    ),
  ).not.toContain("left");
});

test("the remaining limit comes from the server projection", () => {
  expect(
    accountLimitView(account(), reading(), true, projectedLimits("work", 100))
      .remaining,
  ).toBe(33);
  expect(
    accountLimitView(
      account(GeneratedApiAccountStatus.signedOut),
      reading(),
      true,
      projectedLimits("work", 100),
    ).remaining,
  ).toBe(33);
  const failed = accountLimitView(
    account(),
    reading({ data: null, error: "Offline" }),
  );
  expect(failed.remaining).toBeNull();
  expect(failed.error).toBe("Offline");
  // A reading of another account never shows.
  expect(
    accountLimitView(account(), reading({ accountKey: "other" })).remaining,
  ).toBeNull();
  expect(accountLimitView(account(), null).reading).toBe(true);
  // A picker still marks a signed-out account whose limit is used up.
  expect(
    accountLimitView(
      account(GeneratedApiAccountStatus.signedOut),
      reading(),
      false,
      projectedLimits("work", 100),
    ).remaining,
  ).toBe(33);
});

test("only a new server projection changes the shown remaining limit", () => {
  const view = projectedLimits("work", 100);
  vi.useFakeTimers();
  vi.setSystemTime(249_000);
  expect(accountLimitView(account(), reading(), true, view).remaining).toBe(33);
  vi.setSystemTime(250_000);
  expect(accountLimitView(account(), reading(), true, view).remaining).toBe(33);
  const updated = {
    ...view,
    projection: {
      ...view.projection,
      buckets: {
        ...view.projection.buckets,
        current: view.projection.buckets.next,
      },
    },
  };
  expect(
    accountLimitView(account(), reading(), true, updated).remaining,
  ).toBeNull();
  vi.useRealTimers();
});

test("a provider round-trip invalidates the browsed tab and reveals the externally selected account", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  const store = createServerStore(
    createSubscriptions({
      watchResourceChanges: () => () => {},
      watchResourceConnection: () => () => {},
    }),
  );
  const accounts = [
    {
      id: "codex",
      provider: ProviderKind.codex,
      label: "Codex account",
      home: "",
      source: "fixture",
      status: GeneratedApiAccountStatus.ready,
    },
    {
      id: "claude",
      provider: ProviderKind.claude,
      label: "Claude account",
      home: "",
      source: "fixture",
      status: GeneratedApiAccountStatus.ready,
    },
  ] satisfies Account[];
  const render = (value: string) =>
    root.render(
      createElement(
        MantineProvider,
        { env: "test" },
        createElement(
          ServerStoreProvider,
          { value: store },
          createElement(AccountTiles, {
            label: "Accounts",
            accounts,
            value,
            onChange() {},
          }),
        ),
      ),
    );
  try {
    await act(async () => render("codex"));
    const claude = host.querySelector<HTMLInputElement>(
      'input[value="claude"]',
    );
    if (!claude) throw new Error("Missing Claude provider tab");
    await act(async () => claude.click());
    expect(host.querySelector('[data-account-key="claude"]')).not.toBeNull();
    await act(async () => render("claude"));
    await act(async () => render("codex"));
    expect(
      host.querySelector('[data-account-key="codex"][data-selected="true"]'),
    ).not.toBeNull();
    expect(host.querySelector('[data-account-key="claude"]')).toBeNull();
  } finally {
    await act(async () => root.unmount());
    store.dispose();
    vi.unstubAllGlobals();
  }
});

test("mixed-provider multi-selection keeps the browsed tab while secondary accounts change", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  const store = createServerStore(
    createSubscriptions({
      watchResourceChanges: () => () => {},
      watchResourceConnection: () => () => {},
    }),
  );
  const accounts = [
    {
      id: "codex",
      provider: ProviderKind.codex,
      label: "Codex account",
      home: "",
      source: "fixture",
      status: GeneratedApiAccountStatus.ready,
    },
    {
      id: "claude",
      provider: ProviderKind.claude,
      label: "Claude account",
      home: "",
      source: "fixture",
      status: GeneratedApiAccountStatus.ready,
    },
  ] satisfies Account[];
  function Picker() {
    const [value, onChange] = useState(["codex"]);
    return createElement(AccountTiles, {
      label: "Accounts",
      accounts,
      multiple: true,
      value,
      onChange,
    });
  }
  const claudeTile = () => {
    const tile = host.querySelector<HTMLButtonElement>(
      '[data-account-key="claude"]',
    );
    if (!tile) throw new Error("Browsed Claude provider was lost");
    return tile;
  };
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(
            ServerStoreProvider,
            { value: store },
            createElement(Picker),
          ),
        ),
      ),
    );
    const tab = host.querySelector<HTMLInputElement>('input[value="claude"]');
    if (!tab) throw new Error("Missing Claude tab");
    await act(async () => tab.click());
    await act(async () => claudeTile().click());
    expect(claudeTile().dataset.selected).toBe("true");
    await act(async () => claudeTile().click());
    expect(claudeTile().dataset.selected).toBeUndefined();
    expect(host.querySelector('[data-account-key="codex"]')).toBeNull();
  } finally {
    await act(async () => root.unmount());
    store.dispose();
    vi.unstubAllGlobals();
  }
});
