import type {} from "./account-errors.fixture.tsx";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { chromium, webkit } from "playwright-core";
import { createFixtureViteServer } from "../playwright.ts";
import type { Page, Locator } from "@playwright/test";
import type { JsonInput } from "../../src/apiTypes.ts";
import { accountErrorFailure } from "./account-errors-data.ts";
import {
  fixtureUrl,
  fixtureSnapshot,
  type SyncFixture,
} from "./fixture-helpers.ts";

import { execFileSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  test,
  browserExecutablePath,
  handleEntitySyncFixtureRequest,
} from "../playwright.ts";

test("Account settings errors", async () => {
  test.setTimeout(45_000);
  const root = fileURLToPath(new URL("../..//", import.meta.url));
  const negative = process.env.NEGATIVE === "1";
  const browserType = process.env.BROWSER === "webkit" ? webkit : chromium;
  const failure = accountErrorFailure;
  const workspaceId = "0123456789abcdef0123456789abcdef";
  const syncFixture: SyncFixture = {
    snapshot: fixtureSnapshot({
      runtime: {},
      stateDir: "fixture",
      chats: [],
      edges: [],
    }),
    workspaceId,
    epoch: "account-errors-fixture",
  };
  const cacheDir = await mkdtemp(join(tmpdir(), "studio-account-errors-"));
  const server = await createFixtureViteServer({
    configFile: false,
    root,
    cacheDir,
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "account-error-fixture",
        configureServer(server) {
          server.middlewares.use("/fixture-data", (_req, res) => {
            res.setHeader("Content-Type", "application/json");
            res.end(JSON.stringify({ negative }));
          });
          server.middlewares.use((req, res, next) => {
            if (!handleEntitySyncFixtureRequest(req, res, syncFixture)) next();
          });
          server.middlewares.use("/check", (_req, res) => {
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<html><body><div id="root"></div><script type="module" src="/tests/accounts/account-errors.fixture.tsx"></script></body></html>',
            );
          });
        },
        load(id) {
          if (
            negative &&
            id ===
              join(
                root,
                "src/components/accounts/AccountSignIn/AccountSignIn.tsx",
              )
          )
            return execFileSync(
              "git",
              [
                "show",
                "f72c2ce:web/src/components/accounts/AccountSignIn/AccountSignIn.tsx",
              ],
              { cwd: root, encoding: "utf8" },
            );
          return undefined;
        },
      },
    ],
  });
  let browser;
  try {
    await server.listen();
    browser = await browserType.launch({
      headless: true,
      ...(browserType === chromium
        ? {
            executablePath: browserExecutablePath,
          }
        : {}),
    });
    const page = await browser.newPage({
      viewport: { width: 1100, height: 850 },
    });
    page.setDefaultTimeout(5000);
    const errors: string[] = [],
      mutations: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.route("**/api/**", (route) => {
      const pathname = new URL(route.request().url()).pathname;
      if (pathname.startsWith("/api/sync/")) return route.continue();
      if (route.request().method() !== "GET") mutations.push(pathname);
      return route.fulfill({
        json:
          pathname === "/api/accounts"
            ? {
                accounts: [],
                archivedAccounts: [],
                defaultAccountKey: "default",
                logins: [],
              }
            : pathname === "/api/rules"
              ? {
                  rules: [
                    {
                      id: "rule",
                      agent: "lead",
                      name: "Rule diagnostic",
                      kind: "low_workers",
                      error: failure,
                      text: "Saved rule text",
                    },
                  ],
                }
              : pathname === "/api/capabilities"
                ? {
                    agent: "lead",
                    at: 1,
                    errors: [failure],
                    managed: [],
                    mcp: [],
                    model: "gpt-6",
                    nativeInventory: "",
                    observed: [],
                    observedNative: [],
                    role: "lead",
                    servers: [],
                    skills: [],
                  }
                : pathname === "/api/changes"
                  ? { scope: "chat", git: false, error: failure, files: [] }
                  : pathname === "/api/limits"
                    ? {
                        accountKey: "default",
                        error: failure.message,
                        data: null,
                      }
                    : {},
      });
    });
    await page.goto(`${fixtureUrl(server)}/check`);
    if (negative) {
      for (let i = 0; i < 100 && !errors.length; i++)
        await page.waitForTimeout(25);
      assert.ok(
        errors.some((error) =>
          /Objects are not valid as a React child|Minified React error #31/.test(
            error,
          ),
        ),
        "Original sign-in receipt renderer crashes on the structured payload",
      );
      console.log(
        "PASS negative control: original AccountSignIn throws React object-child error",
      );
    } else {
      async function details(
        expected: JsonInput,
        scope: Page | Locator = page,
      ) {
        const button = scope
          .getByRole("button", { name: "Error details", exact: true })
          .first();
        await button.click();
        assert.deepEqual(
          JSON.parse(
            (await scope
              .getByRole("button", {
                name: "Hide error details",
                exact: true,
              })
              .first()
              .locator("..")
              .locator(":scope > span")
              .last()
              .textContent())!,
          ),
          expected,
        );
        assert.equal(await page.locator("[data-shell]").count(), 1);
      }
      await page
        .getByText(failure.message, { exact: true })
        .waitFor({ state: "attached" });
      for (const section of ["rules", "changes", "tools"]) {
        await page.evaluate((value) => window.showSurface(value), section);
        if (section === "tools")
          await page
            .getByText("Some tools could not load. View details.", {
              exact: true,
            })
            .click();
        await page.getByText(failure.message, { exact: true }).waitFor();
        await details(failure);
      }
      assert.deepEqual(errors, []);
      assert.deepEqual(
        mutations,
        [],
        "Rendering error details never submits a form or repeats an operation",
      );
      console.log(
        `PASS ${browserType === chromium ? "Chromium" : "WebKit"}: sign-in, rules, changes and tool diagnostics retain full structured fields without losing the shell`,
      );
    }
  } finally {
    await browser?.close();
    await server.close();
    await rm(cacheDir, { recursive: true, force: true });
  }
});
