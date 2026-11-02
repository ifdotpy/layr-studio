import { AccountStatus as GeneratedApiAccountStatus } from "../generated/api.ts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test, vi } from "vitest";
import { projectedLimits } from "../usage/accountLimits.fixture";
import type { AccountLimitsSnapshot } from "../usage/accountUsage";
import type { Account } from "./Accounts";
import { accountLimitView, AccountTileLabel } from "./AccountTiles";

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

test("the remaining limit is known only for a signed-in account with data", () => {
  expect(
    accountLimitView(account(), reading(), true, projectedLimits("work", 100))
      .remaining,
  ).toBe(33);
  expect(
    accountLimitView(account(GeneratedApiAccountStatus.signedOut), reading())
      .remaining,
  ).toBe(null);
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
