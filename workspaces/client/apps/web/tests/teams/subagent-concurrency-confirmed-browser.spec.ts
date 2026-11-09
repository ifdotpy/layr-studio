import { agentSseServer } from "../fixtures/agent-sse-server.ts";
import type {} from "./subagent-concurrency-confirmed-browser.fixture.tsx";
import {
  initialConfirmedLead,
  confirmedWorkspaceId,
} from "./concurrency-fixture-data.ts";
import type { components } from "../../src/generated/api.ts";

import { fixturePostBody } from "./helpers.ts";
import { createFixtureViteServer as createServer } from "../playwright.ts";
import {
  API_SCHEMA_HASH_HEADER,
  readApiSchemaHash,
  test,
  expect,
} from "../playwright.ts";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

test("subagent concurrency uses the confirmed response", async ({ page }) => {
  test.setTimeout(60000);
  const root = join(import.meta.dirname, "../../");
  const cacheDir = await mkdtemp(
    join(tmpdir(), "studio-concurrency-confirmed-"),
  );
  const workspaceId = confirmedWorkspaceId;
  const initial = initialConfirmedLead;
  const stream = agentSseServer(workspaceId);
  const server = await createServer({
    configFile: false,
    root,
    cacheDir,
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "concurrency-confirmed-fixture",
        configureServer(server) {
          server.middlewares.use((req, res, next) => {
            if (!stream.handle(req, res)) next();
          });
          server.middlewares.use("/check", (_req, res) => {
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<div id="root"></div><script type="module" src="/tests/teams/subagent-concurrency-confirmed-browser.fixture.tsx"></script>',
            );
          });
        },
        load(id) {
          if (
            process.env.CONFIRMATION_BASELINE &&
            id ===
              join(
                root,
                "src/components/agents/SubagentConcurrencyControl/SubagentConcurrencyControl.tsx",
              )
          )
            return execFileSync(
              "git",
              [
                "show",
                `${process.env.CONFIRMATION_BASELINE}:web/src/components/agents/SubagentConcurrencyControl/SubagentConcurrencyControl.tsx`,
              ],
              { cwd: root, encoding: "utf8" },
            );
          return undefined;
        },
      },
    ],
  });
  const errors: string[] = [];
  const recoveryReads: string[] = [];
  page.on("request", (request) => {
    if (
      /\/(?:api\/session|api\/sync\/pull|snapshot)(?:\?|$)/.test(request.url())
    )
      recoveryReads.push(request.url());
  });
  page.on("pageerror", (error) => errors.push(error.message));
  let releaseReply: (() => void) | undefined;
  const writes: components["schemas"]["ConversationRequest"][] = [];
  let reply = { ...initial, concurrency: 64, agentModeRevision: 1 };
  await page.route("**/api/conversation", async (route) => {
    writes.push(fixturePostBody(route, "/api/conversation"));
    await new Promise<void>((resolve) => (releaseReply = resolve));
    await route.fulfill({
      headers: { [API_SCHEMA_HASH_HEADER]: readApiSchemaHash() },
      json: reply,
    });
  });
  try {
    await server.listen();
    await page.goto(`${server.resolvedUrls!.local[0]!}check`);
    const control = page.locator(".agent-mode-control");
    const input = page.getByRole("textbox", {
      name: "Subagent parallelism",
    });
    await expect(control).toHaveAttribute("data-concurrency", "32");
    await input.fill("64");
    await input.blur();
    await expect.poll(() => writes.length).toBe(1);
    assert.equal(writes[0]!.expected_mode_revision, 0);
    await expect(control).toHaveAttribute("data-concurrency", "32");
    await expect(input).toBeDisabled();
    assert.equal(await page.evaluate(() => window.refreshCount), 0);
    releaseReply!();
    await expect(input).toBeEnabled();
    assert.equal(await page.evaluate(() => window.refreshCount), 0);
    await expect(control).toHaveAttribute("data-concurrency", "64");
    await expect(control).toHaveAttribute("data-agent-mode-revision", "1");
    await expect(input).toBeEnabled({ timeout: 1000 });
    assert.equal(writes[0]!.expected_account_key, "default");
    await expect(control.getByText("Applying…", { exact: true })).toHaveCount(
      0,
    );
    await expect(control.getByRole("alert")).toHaveCount(0);
    reply = { ...initial, concurrency: 0, agentModeRevision: 2 };
    await input.fill("0");
    await input.blur();
    await expect.poll(() => writes.length).toBe(2);
    await expect(control).toHaveAttribute("data-agent-mode", "multi");
    await expect(input).toBeDisabled();
    releaseReply!();
    await expect(input).toBeEnabled();
    await expect(control).toHaveAttribute("data-agent-mode", "single");
    await expect(input).toBeEnabled();
    assert.equal(writes[1]!.expected_mode_revision, 1);
    // A newer snapshot remains authoritative when an older response arrives.
    reply = { ...initial, concurrency: 128, agentModeRevision: 3 };
    await input.fill("128");
    await input.blur();
    await expect.poll(() => writes.length).toBe(3);
    await page.evaluate((lead) => window.setLead!(lead), {
      ...initial,
      concurrency: 8,
      agentModeRevision: 4,
    });
    await expect(control).toHaveAttribute("data-concurrency", "8");
    await expect(control).toHaveAttribute("data-agent-mode-revision", "4");
    releaseReply!();
    await expect(input).toBeEnabled();
    await expect(control).toHaveAttribute("data-concurrency", "8");
    await expect(control).toHaveAttribute("data-agent-mode-revision", "4");
    await expect(input).toHaveValue("8");
    assert.deepEqual(
      recoveryReads,
      [],
      "Successful mutations need no recovery reads",
    );
    assert.deepEqual(errors, []);
  } finally {
    releaseReply?.();
    await server.close();
    await rm(cacheDir, { recursive: true, force: true });
  }
});
