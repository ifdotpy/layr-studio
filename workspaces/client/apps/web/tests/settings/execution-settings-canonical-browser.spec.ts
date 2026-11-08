import type { components } from "../../src/generated/api.ts";
import type { CanonicalReset } from "./execution-settings-canonical-browser.fixture.tsx";
import type {} from "./execution-settings-canonical-browser.fixture.tsx";
import { fixtureUrl } from "./fixture-helpers.ts";
import {
  API_SCHEMA_HASH_HEADER,
  readApiSchemaHash,
  apiSchemaHandshakeSse,
} from "../playwright.ts";
import { setupControl, chooseSetupValue } from "../fixtures/setup-controls.ts";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { modelValue, selectModel } from "../fixtures/model-picker.ts";
import { test, expect } from "../playwright.ts";

test("execution settings canonical browser", async ({ page }) => {
  test.setTimeout(180000);
  const root = join(import.meta.dirname, "../../");
  const { createServer } = await import("vite");
  const cacheDir = await mkdtemp(join(tmpdir(), "studio-settings-canonical-"));
  const server = await createServer({
    configFile: false,
    root,
    cacheDir,
    server: { host: "127.0.0.1", port: 0 },
    optimizeDeps: {
      noDiscovery: true,
      include: [
        "react",
        "react/jsx-runtime",
        "react/jsx-dev-runtime",
        "react-dom/client",
        "react-dom",
        "@mantine/core",
        "lucide-react",
        "@mantine/hooks",
      ],
    },
    plugins: [
      {
        name: "settings-canonical-fixture",
        configureServer(server) {
          server.middlewares.use("/check", (_req, res) => {
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<div id="root"></div><script type="module" src="/tests/settings/execution-settings-canonical-browser.fixture.tsx"></script>',
            );
          });
        },
        load(id) {
          if (
            (process.env.BASELINE === "1" ||
              process.env.CONFIRMATION_BASELINE) &&
            id === join(root, "src/components/agents/ExecutionSettings.tsx")
          )
            return execFileSync(
              "layr",
              [
                "show",
                `${process.env.CONFIRMATION_BASELINE || "bf59170a063271b7933801a1b7b6b2f6"}:workspaces/client/apps/web/src/components/agents/ExecutionSettings.tsx`,
              ],
              { cwd: root, encoding: "utf8" },
            );
          return undefined;
        },
      },
    ],
  });
  const failures: string[] = [];
  try {
    await server.listen();
    await page.setViewportSize({ width: 390, height: 700 });
    const errors: string[] = [];
    page.on("pageerror", (error) => {
      errors.push(error.message);
      console.error(error.message);
    });
    await page.route("**/api/sync/stream?**", (route) => {
      const resources = JSON.parse(
        new URL(route.request().url()).searchParams.get("resources") || "[]",
      );
      return route.fulfill({
        contentType: "text/event-stream",
        body: apiSchemaHandshakeSse(
          `event: resources\ndata: ${JSON.stringify({ protocol: 3, workspaceId: "1234567890abcdef1234567890abcdef", epoch: "fixture", revision: 1, reason: "initial", resources })}\n\n`,
        ),
      });
    });
    await page.route("**/api/sync/identity", (route) =>
      route.fulfill({
        headers: { [API_SCHEMA_HASH_HEADER]: readApiSchemaHash() },
        json: {
          workspaceId: "1234567890abcdef1234567890abcdef",
          syncProtocol: 2,
        } satisfies components["schemas"]["SyncIdentityResponse"],
      }),
    );
    await page.goto(fixtureUrl(server, "check"));
    await page.waitForFunction(() => !!window.canonicalReset);
    const reset = async (options?: CanonicalReset) => {
      await page.evaluate((options) => {
        localStorage.clear();
        window.canonicalReset!(options);
      }, options);
      await page
        .getByRole("button", {
          name: options?.defaults ? "Subagent defaults" : "Main agent settings",
          exact: true,
        })
        .click();
    };
    const effort = (defaults = false) =>
      setupControl(
        page.getByLabel(
          defaults ? "Default subagent reasoning" : "Main agent reasoning",
          { exact: true },
        ),
      );
    const value = async (expected: string, defaults = false) => {
      await expect(
        page.getByRole("radiogroup", {
          name: defaults
            ? "Default subagent reasoning"
            : "Main agent reasoning",
          exact: true,
        }),
      ).toHaveAttribute("data-value", expected);
    };
    const check = async (name: string, run: () => Promise<void>) => {
      if (process.env.CASE && process.env.CASE !== name) return;
      try {
        await run();
        console.log(`PASS ${name}`);
      } catch (error) {
        failures.push(`${name}: ${(error as Error).message}`);
        console.error(`FAIL ${name}: ${(error as Error).message}`);
        console.error(
          JSON.stringify(
            await page.evaluate(() => ({
              calls: window.settingsCalls,
              transfers: window.transfers,
              refreshCount: window.refreshCount,
              catalogRequests: window.catalogRequests,
              text: document.querySelector<HTMLElement>('[role="dialog"]')
                ?.innerText,
            })),
          ),
        );
      }
    };
    await check("canonical", async () => {
      await reset();
      await page.evaluate(
        () =>
          (window.reply = {
            ...window.agent,
            effort: "medium",
          }),
      );
      await effort().selectOption("high");
      await value("medium");
      await expect(
        page.getByRole("dialog", { name: "Settings change", exact: true }),
      ).toContainText("medium");
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          tail: "Unrelated snapshot",
        }),
      );
      await value("medium");
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          effort: "medium",
        }),
      );
      await page.waitForTimeout(50);
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          effort: "max",
        }),
      );
      await value("max");
    });
    await check("confirmed-response-before-refresh", async () => {
      await reset();
      await page.evaluate(() => {
        window.reply = {
          ...window.agent,
          effort: "medium",
        };
        window.refreshHold = true;
      });
      await effort().selectOption("high");
      await page.waitForFunction(() => !!window.canonicalRefreshRelease);
      await value("medium");
      assert.equal(await effort().isDisabled(), false);
      assert.equal(await page.getByText("Saving…", { exact: true }).count(), 0);
      assert.equal(await page.evaluate(() => window.refreshCount), 1);
      await page.evaluate(() => {
        window.refreshFailure = true;
        window.canonicalRefreshRelease!();
      });
      await page
        .getByRole("alert")
        .filter({ hasText: "Settings saved. Snapshot unavailable" })
        .waitFor();
      await value("medium");
      assert.equal(await effort().isDisabled(), false);
    });
    await check("server-confirmation-required", async () => {
      await reset();
      await page.evaluate(() => {
        window.hold = true;
        window.reply = {
          ...window.agent,
          effort: "medium",
        };
      });
      await effort().selectOption("high");
      await page.waitForFunction(() => !!window.canonicalRelease);
      await value("low");
      assert.equal(await effort().isDisabled(), true);
      assert.equal(await page.getByText("Saved", { exact: true }).count(), 0);
      assert.equal(await page.evaluate(() => window.refreshCount), 0);
      await page.evaluate(() => window.canonicalRelease!());
      await value("medium");
      assert.equal(await effort().isDisabled(), false);
    });
    await check("model-and-modes-confirmation-required", async () => {
      for (const field of ["model", "fastMode", "daybreakEnabled"]) {
        await reset();
        const target = field === "model" ? "gpt-5.6-sol" : true;
        await page.evaluate(
          ({ field, target }) => {
            window.hold = true;
            window.refreshHold = true;
            window.reply = {
              ...window.agent,
              [field]: target,
            };
            window.canonicalRelease = null;
            window.canonicalRefreshRelease = null;
          },
          { field, target },
        );
        const model = page.getByLabel("Main agent model", { exact: true });
        const toggle = page.getByRole("switch", {
          name: field === "fastMode" ? "Fast mode" : "Daybreak",
          exact: true,
        });
        if (field === "model") await selectModel(model, target as string);
        else await toggle.click();
        await page.waitForFunction(() => !!window.canonicalRelease);
        if (field === "model")
          assert.equal(await modelValue(model), "gpt-6-astra");
        else await expect(toggle).not.toBeChecked();
        assert.equal(await page.evaluate(() => window.refreshCount), 0);
        await page.evaluate(() => window.canonicalRelease!());
        await page.waitForFunction(() => !!window.canonicalRefreshRelease);
        if (field === "model")
          await expect.poll(() => modelValue(model)).toBe(target);
        else await expect(toggle).toBeChecked();
        await expect(model).not.toHaveAttribute("aria-disabled", "true");
        assert.equal(await effort().isDisabled(), false);
        await page.evaluate(() => window.canonicalRefreshRelease!());
      }
    });
    await check("review-and-permissions-confirmation-required", async () => {
      await reset({ defaults: true });
      await page.getByText("Review", { exact: true }).click();
      await page.evaluate(() => {
        window.hold = true;
        window.refreshHold = true;
        window.canonicalRelease = null;
        window.canonicalRefreshRelease = null;
        window.reply = {
          ...window.agent,
          reviewDefaults: { model: "gpt-5.6-sol", effort: "high" },
        };
      });
      const reviewModel = page.getByLabel("Default review model", {
        exact: true,
      });
      await selectModel(reviewModel, "gpt-5.6-luna");
      await page.waitForFunction(() => !!window.canonicalRelease);
      assert.equal(await modelValue(reviewModel), "__model_default__");
      await page.evaluate(() => window.canonicalRelease!());
      await page.waitForFunction(() => !!window.canonicalRefreshRelease);
      await expect.poll(() => modelValue(reviewModel)).toBe("gpt-5.6-sol");
      await expect(reviewModel).not.toHaveAttribute("aria-disabled", "true");
      await page.evaluate(() => window.canonicalRefreshRelease!());
      await reset();
      await page.keyboard.press("Escape");
      await page.getByText("Permissions", { exact: true }).click();
      await page.evaluate(() => {
        window.hold = true;
        window.refreshHold = true;
        window.canonicalRelease = null;
        window.canonicalRefreshRelease = null;
        window.reply = {
          ...window.agent,
          permissionMode: "bypassPermissions",
        };
      });
      const permission = page.getByRole("switch", {
        name: "Full access without approval",
        exact: true,
      });
      await permission.click();
      await page.waitForFunction(() => !!window.canonicalRelease);
      await expect(permission).not.toBeChecked();
      await expect(permission).toBeDisabled();
      await page.evaluate(() => window.canonicalRelease!());
      await page.waitForFunction(() => !!window.canonicalRefreshRelease);
      await expect(permission).toBeChecked();
      await expect(permission).toBeEnabled();
      await page.evaluate(() => window.canonicalRefreshRelease!());
    });
    await check("transfer-response-before-old-summary", async () => {
      await reset({
        defaults: true,
        agent: {
          accountTransfer: {
            id: "pending-transfer",
            scope: "subagents",
            targetAccountKey: "second",
            status: "pending",
            updated: 1,
            waitingCount: 2,
          },
        },
      });
      await page.evaluate(() => {
        window.transferHold = true;
        window.refreshHold = true;
        window.transferReply = {
          id: "pending-transfer",
          leadId: "first",
          targetAccountKey: "second",
          scope: "subagents",
          status: "cancelled",
          updated: 2,
        };
      });
      await page
        .getByRole("button", { name: "Cancel remaining", exact: true })
        .click();
      await page.waitForFunction(() => !!window.transferRelease);
      await expect(page.getByText("2 waiting", { exact: true })).toBeVisible();
      await page.evaluate(() => window.transferRelease!());
      await page.waitForFunction(() => !!window.canonicalRefreshRelease);
      await expect(
        page.getByRole("button", { name: "Cancel remaining", exact: true }),
      ).toHaveCount(0);
      await expect(page.getByText("2 waiting", { exact: true })).toHaveCount(0);
      assert.equal(await effort(true).isDisabled(), false);
      await page.evaluate(() => {
        window.setAgent!({
          ...window.agent,
          accountTransfer: {
            ...window.agent.accountTransfer,
            waitingCount: 9,
          },
        });
      });
      await expect(page.getByText("9 waiting", { exact: true })).toHaveCount(0);
      await page.evaluate(() => {
        window.setAgent!({
          ...window.agent,
          accountTransfer: {
            ...window.agent.accountTransfer,
            updated: 3,
            waitingCount: 1,
          },
        });
      });
      await expect(page.getByText("1 waiting", { exact: true })).toBeVisible();
      await page.evaluate(() => window.canonicalRefreshRelease!());
    });
    await check("replay", async () => {
      await reset({ agent: { status: "running" } });
      await page.evaluate(() => (window.canonicalFail = "network"));
      await effort().selectOption("high");
      await page.getByRole("button", { name: "Check settings save" }).waitFor();
      const request = await page.evaluate(() => window.settingsCalls[0]);
      await page.reload();
      await page.waitForFunction(() => !!window.canonicalReset);
      await page
        .getByRole("button", { name: "Main agent settings", exact: true })
        .click();
      await page.getByRole("button", { name: "Check settings save" }).waitFor();
      await page.evaluate(
        () =>
          (window.reply = {
            ...window.agent,
            pendingSettings: {
              model: "gpt-6-astra",
              effort: "max",
              fastMode: true,
            },
          }),
      );
      await page.getByRole("button", { name: "Check settings save" }).click();
      await value("max");
      assert.deepEqual(
        await page.evaluate(() => window.settingsCalls[0]),
        request,
      );
      assert.equal(
        await page.getByRole("button", { name: "Check settings save" }).count(),
        0,
      );
      await page.reload();
      await page
        .getByRole("button", { name: "Main agent settings", exact: true })
        .click();
      assert.equal(
        await page.getByRole("button", { name: "Check settings save" }).count(),
        0,
      );
    });
    await check("account-scope", async () => {
      await reset({ agent: { status: "running" } });
      await page.evaluate(() => (window.canonicalFail = "network"));
      await effort().selectOption("high");
      await page.getByRole("button", { name: "Check settings save" }).waitFor();
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          accountKey: "second",
          effort: "medium",
        }),
      );
      const button = page.getByRole("button", {
        name: "Main agent settings",
        exact: true,
      });
      if ((await button.getAttribute("aria-expanded")) === "false")
        await button.click();
      await value("medium");
      assert.equal(
        await page.getByRole("button", { name: "Check settings save" }).count(),
        0,
      );
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          accountKey: "default",
          effort: "low",
        }),
      );
      if ((await button.getAttribute("aria-expanded")) === "false")
        await button.click();
      await page.getByRole("button", { name: "Check settings save" }).waitFor();
      await value("low");
    });
    await check("late-scope-response", async () => {
      await reset();
      await page.evaluate(() => {
        window.hold = true;
        window.reply = {
          ...window.agent,
          effort: "high",
        };
      });
      await effort().selectOption("high");
      await page.waitForFunction(() => !!window.canonicalRelease);
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          id: "second",
          effort: "medium",
        }),
      );
      await page.evaluate(() => window.canonicalRelease!());
      const button = page.getByRole("button", {
        name: "Main agent settings",
        exact: true,
      });
      if ((await button.getAttribute("aria-expanded")) === "false")
        await button.click();
      await value("medium");
      await page.waitForTimeout(50);
      assert.equal(await page.evaluate(() => window.refreshCount), 0);
    });
    await check("defaults-and-refresh-failure", async () => {
      await reset({ defaults: true });
      await page.evaluate(() => {
        window.reply = {
          ...window.agent,
          workerDefaults: {
            model: "gpt-5.6-luna",
            effort: "medium",
            fastMode: false,
          },
        };
        window.refreshFailure = true;
      });
      await effort(true).selectOption("max");
      await value("medium", true);
      assert.equal(
        await page.evaluate(
          () => window.settingsCalls[0]!.expected_account_key,
        ),
        "default",
      );
      await page
        .getByRole("alert")
        .filter({ hasText: "Settings saved." })
        .waitFor();
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          workerDefaults: {
            model: "gpt-5.6-luna",
            effort: "medium",
            fastMode: false,
          },
        }),
      );
      await page.waitForTimeout(50);
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          workerDefaults: {
            model: "gpt-5.6-luna",
            effort: "low",
            fastMode: false,
          },
        }),
      );
      await value("low", true);
    });
    await check("review-defaults", async () => {
      await reset({ defaults: true });
      await page.getByText("Review", { exact: true }).click();
      const reviewModel = page.locator("#review-model");
      await page.evaluate(() => {
        window.reply = {
          ...window.agent,
          reviewDefaults: { model: "gpt-5.6-luna", effort: null },
        };
      });

      await page
        .locator('input[type="radio"][data-value="gpt-5.6-luna"]:visible')
        .click();
      await page.waitForFunction(() => window.settingsCalls.length === 1);
      assert.deepEqual(
        await page.evaluate(() => window.settingsCalls[0]!.review_defaults),
        {
          model: "gpt-5.6-luna",
          effort: null,
        },
      );
      assert.equal(await modelValue(reviewModel), "gpt-5.6-luna");
      await page.evaluate(() => {
        window.setAgent!({
          ...window.agent,
          reviewDefaults: { model: "gpt-5.6-luna", effort: null },
        });
        window.reply = {
          ...window.agent,
          reviewDefaults: { model: "gpt-5.6-luna", effort: "high" },
        };
      });
      await chooseSetupValue(
        page.getByLabel("Default review reasoning", { exact: true }),
        "high",
      );
      await page.waitForFunction(() => window.settingsCalls.length === 2);
      assert.deepEqual(
        await page.evaluate(() => window.settingsCalls[1]!.review_defaults),
        {
          model: "gpt-5.6-luna",
          effort: "high",
        },
      );
    });
    await check("rejected-does-not-pin-old-snapshot", async () => {
      await reset();
      await page.evaluate(() => {
        window.hold = true;
        window.canonicalFail = "rejected";
      });
      await effort().selectOption("high");
      await page.waitForFunction(() => !!window.canonicalRelease);
      await page.evaluate(() => {
        window.setAgent!({
          ...window.agent,
          effort: "medium",
        });
        window.canonicalRelease!();
      });
      await page.getByRole("alert").waitFor();
      await value("medium");
    });
    await check("rejected-after-confirmed-before-snapshot", async () => {
      await reset();
      await page.evaluate(
        () =>
          (window.reply = {
            ...window.agent,
            effort: "medium",
          }),
      );
      await effort().selectOption("high");
      await value("medium");
      await page.evaluate(() => (window.canonicalFail = "rejected"));
      await effort().selectOption("max");
      await page.getByRole("alert").waitFor();
      await value("medium");
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          effort: "max",
        }),
      );
      await value("max");
    });
    await check("newer-snapshot-handoff", async () => {
      await reset();
      await page.evaluate(
        () =>
          (window.reply = {
            ...window.agent,
            effort: "medium",
          }),
      );
      await effort().selectOption("high");
      await value("medium");
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          effort: "max",
        }),
      );
      await value("max");
    });
    await check("stale-account-submission", async () => {
      await reset({ agent: { status: "running" } });
      await page.evaluate(() => (window.serverAccount = "second"));
      await effort().selectOption("high");
      await page
        .getByRole("alert")
        .filter({ hasText: "The account changed." })
        .waitFor();
      await value("low");
      assert.equal(
        await page.evaluate(
          () => window.settingsCalls[0]!.expected_account_key,
        ),
        "default",
      );
      assert.equal(
        await page.getByRole("button", { name: "Check settings save" }).count(),
        0,
      );
    });
    await check("legacy-receipt-not-replayed", async () => {
      await reset();
      const legacy = {
        id: "first",
        next_turn: true,
        request_id: "legacy-save",
        model: "gpt-6-astra",
        effort: "high",
        fast_mode: false,
      };
      await page.evaluate((legacy) => {
        localStorage.setItem(
          "next-turn-settings:first",
          JSON.stringify(legacy),
        );
        window.canonicalReset!({
          agent: { accountKey: "second", effort: "medium" },
        });
      }, legacy);
      await page
        .getByRole("button", { name: "Main agent settings", exact: true })
        .click();
      await value("medium");
      assert.equal(
        await page.getByRole("button", { name: "Check settings save" }).count(),
        0,
      );
      assert.deepEqual(await page.evaluate(() => window.settingsCalls), []);
      assert.deepEqual(
        await page.evaluate(() =>
          JSON.parse(
            localStorage.getItem("next-turn-settings:first") || "null",
          ),
        ),
        legacy,
      );
      assert.equal(
        await page
          .getByRole("button", { name: "Discard previous settings request" })
          .count(),
        0,
      );
    });
    await check("permission-canonical-handoff", async () => {
      await reset();
      await page.keyboard.press("Escape");
      await page.getByText("Permissions", { exact: true }).click();
      await page.evaluate(() => {
        window.reply = {
          ...window.agent,
          permissionMode: "bypassPermissions",
        };
        window.refreshFailure = true;
      });
      const permission = page.getByRole("switch", {
        name: "Full access without approval",
        exact: true,
      });
      await permission.click();
      await page
        .getByRole("alert")
        .filter({ hasText: "Settings saved." })
        .waitFor();
      assert.equal(await permission.isChecked(), true);
      assert.equal(
        await page.evaluate(
          () => window.settingsCalls[0]!.expected_account_key,
        ),
        "default",
      );
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          tail: "Unrelated snapshot",
        }),
      );
      assert.equal(await permission.isChecked(), true);
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          permissionMode: "bypassPermissions",
        }),
      );
      await page.waitForTimeout(50);
      await page.evaluate(() =>
        window.setAgent!({
          ...window.agent,
          permissionMode: "default",
        }),
      );
      assert.equal(await permission.isChecked(), false);
      await page.evaluate(() => {
        window.reply = {
          ...window.agent,
          permissionMode: "default",
        };
        window.refreshFailure = false;
      });
      await permission.click();
      await page.waitForFunction(() => window.refreshCount === 2);
      assert.equal(
        await page.evaluate(() => window.settingsCalls[1]!.yolo_mode),
        true,
      );
      assert.equal(await permission.isChecked(), false);
    });
    expect(errors).toEqual([]);
    assert.deepEqual(failures, []);
    console.log(
      `PASS execution settings canonical state (${process.env.BROWSER ?? "chromium"})`,
    );
  } finally {
    await server.close();
    await rm(cacheDir, { recursive: true, force: true });
  }
});
