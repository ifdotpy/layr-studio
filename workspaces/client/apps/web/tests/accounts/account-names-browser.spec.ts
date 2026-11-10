import type {} from "./account-names.fixture.tsx";
import type { Page } from "@playwright/test";
import type { Account, AccountsState } from "./fixture-helpers.ts";
import {
  fixtureUrl,
  fixtureAccount,
  fixtureAccounts,
  fixtureRequest,
  type FixtureRequest,
} from "./fixture-helpers.ts";

import { join } from "node:path";
import { test, expect, createFixtureViteServer } from "../playwright.ts";

async function fixture(page: Page) {
  page.on("pageerror", (error) =>
    console.error("Account fixture:", error.message),
  );
  const root = join(import.meta.dirname, "../../");
  const accounts: Account[] = (
    [
      {
        id: "default",
        home: "/fixture/codex",
        source: "Codex",
        provider: "codex",
        label: "Personal",
        email: "personal@example.com",
        status: "signedOut",
      },
      {
        id: "work",
        home: "/fixture/claude",
        source: "Claude Code",
        provider: "claude",
        label: "Work 1",
        email: "work@example.com",
        status: "signedOut",
      },
    ] satisfies Parameters<typeof fixtureAccount>[0][]
  ).map(fixtureAccount);
  const data = (): AccountsState =>
    fixtureAccounts({
      accounts,
      archivedAccounts: [],
      defaultAccountKey: "default",
      logins: [],
    });
  const bodies: Array<FixtureRequest["body"]> = [];
  let loseResponse = false;
  await page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/api/sync/identity")
      return route.fulfill({ status: 404, json: {} });
    if (path === "/api/accounts/name") {
      const body = fixtureRequest(
        "/api/accounts/name",
        route.request().postDataJSON(),
      ).body;
      bodies.push(body);
      accounts.find((account) => account.id === body.account_key)!.label =
        body.label.trim();
      if (loseResponse) {
        loseResponse = false;
        return route.abort("failed");
      }
    }
    if (path === "/api/claude/profiles") {
      bodies.push(
        fixtureRequest("/api/claude/profiles", route.request().postDataJSON())
          .body,
      );
      return route.fulfill({ json: data() });
    }
    await route.fulfill({
      json:
        path === "/api/accounts" || path === "/api/accounts/name" ? data() : {},
    });
  });
  const server = await createFixtureViteServer({
    configFile: false,
    root,
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "account-names-fixture",
        configureServer(server) {
          server.middlewares.use("/fixture-data", (_req, res) => {
            res.setHeader("Content-Type", "application/json");
            res.end(JSON.stringify(data()));
          });
          server.middlewares.use("/check", (_req, res) => {
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<div id="root"></div><script type="module" src="/tests/accounts/account-names.fixture.tsx"></script>',
            );
          });
        },
      },
    ],
  });
  try {
    await server.listen();
    await page.goto(`${fixtureUrl(server)}/check`);
  } catch (error) {
    await server.close();
    throw error;
  }
  return {
    bodies,
    loseNextResponse: () => {
      loseResponse = true;
    },
    close: () => server.close(),
    publish: () =>
      page.evaluate(
        (serialized) => window.publishAccounts(JSON.parse(serialized)),
        JSON.stringify(data()),
      ),
  };
}

async function rename(page: Page, identity: string) {
  const row = page.locator('[data-account-identity="' + identity + '"]');
  await row.getByRole("button", { name: /^Actions for/ }).click();
  await row.getByRole("menuitem", { name: "Rename", exact: true }).click();
  return page.getByRole("dialog", { name: /^Rename/ });
}

test("settings account rename saves exact keys and shows errors across themes", async ({
  page,
}) => {
  const f = await fixture(page);
  try {
    let dialog = await rename(page, "codex:personal@example.com");
    await dialog
      .getByRole("textbox", { name: "Name", exact: true })
      .fill("Work 2");
    await dialog.getByRole("button", { name: "Save", exact: true }).click();
    await expect(dialog).not.toBeVisible();
    expect(f.bodies[0]).toMatchObject({
      account_key: "default",
      label: "Work 2",
    });
    await f.publish();
    await expect(
      page.locator(
        '[data-account-identity="codex:personal@example.com"] .account-identity strong',
      ),
    ).toHaveText("Work 2");
    dialog = await rename(page, "claude:work@example.com");
    await dialog
      .getByRole("textbox", { name: "Name", exact: true })
      .fill("Team");
    f.loseNextResponse();
    await dialog.getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByRole("alert").first()).toBeVisible();
    await expect(dialog).toBeVisible();
    expect(f.bodies.at(-1)).toMatchObject({
      account_key: "work",
      label: "Team",
    });
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    for (const [color, width] of [
      ["dark", 1280],
      ["light", 1280],
      ["dark", 390],
      ["light", 390],
    ] as const) {
      await page.setViewportSize({ width, height: 1000 });
      await page.evaluate((value) => window.setTheme(value), color);
      await expect(page.locator("html")).toHaveAttribute(
        "data-mantine-color-scheme",
        color,
      );
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
    }
  } finally {
    await f.close();
  }
});

test("settings rename Escape closes the modal and keeps the account list visible", async ({
  page,
}) => {
  const f = await fixture(page);
  try {
    const dialog = await rename(page, "codex:personal@example.com");
    await dialog
      .getByRole("textbox", { name: "Name", exact: true })
      .fill("Discard");
    await dialog
      .getByRole("textbox", { name: "Name", exact: true })
      .press("Escape");
    await expect(dialog).not.toBeVisible();
    await expect(
      page.getByRole("region", { name: "Accounts", exact: true }),
    ).toBeVisible();
    expect(f.bodies).toHaveLength(0);
  } finally {
    await f.close();
  }
});
