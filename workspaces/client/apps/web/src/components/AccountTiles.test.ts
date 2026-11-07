import {
  AccountStatus as GeneratedApiAccountStatus,
  ProviderKind,
} from "../generated/api.ts";
import { createElement } from "react";
import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test, vi } from "vitest";
import { projectedLimits } from "../usage/accountLimits.fixture";
import type { AccountLimitsSnapshot } from "../usage/accountUsage";
import type { Account } from "./Accounts";
import {
  accountLimitView,
  AccountTileLabel,
  AccountTiles,
} from "./AccountTiles";

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
