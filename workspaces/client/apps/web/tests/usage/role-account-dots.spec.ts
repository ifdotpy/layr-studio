import assert from "node:assert/strict";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";

// The footer shows one allowance dot per role (Lead, Workers, Review), not one
// per account. With subagents off only the lead works: one dot, and neither
// the composer summary nor the model picker offers Workers.
test("footer dots follow the chat roles and subagents off leaves the lead", async ({
  browser,
}) => {
  test.setTimeout(120_000);
  const root = await mkdtemp(join(tmpdir(), "studio-role-dots-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, root], {
    stdio: ["pipe", "pipe", "pipe"],
    env: { ...process.env, FIXTURE_WORKER_COUNT: "0" },
  });
  let log = "";
  fixture.stderr?.on("data", (chunk) => (log += chunk));
  const port = await new Promise<number>((resolve, reject) => {
    fixture.stdout?.once("data", (chunk) =>
      resolve(Number(String(chunk).trim())),
    );
    fixture.once("exit", () => reject(new Error(log)));
  });
  const origin = `http://127.0.0.1:${port}`;
  const { token } = await (await fetch(`${origin}/api/session`)).json();
  for (let index = 1; index <= 6; index++) {
    const home = join(root, `account-${index}`);
    await mkdir(home);
    await writeFile(
      join(home, "auth.json"),
      JSON.stringify({ OPENAI_API_KEY: "fixture-key-never-sent" }),
    );
    const response = await fetch(`${origin}/api/accounts/register`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Canvas-Token": token },
      body: JSON.stringify({ home }),
    });
    assert.ok(response.ok, await response.text());
  }
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 },
  });
  try {
    await page.goto(origin);
    await page.locator("[data-chat]", { hasText: "Release lead" }).click();
    await page.locator("#message").waitFor();
    const dots = page.locator(".account-limits-dot-target");
    const roles = () =>
      dots.evaluateAll((nodes) =>
        nodes.map((node) => (node as HTMLElement).dataset.role),
      );
    await expect.poll(roles).toEqual(["Lead", "Workers", "Review"]);
    const segments = page.locator(
      '#composer .execution-summary-segment[data-role="worker"]',
    );
    await expect(segments).toHaveCount(1);

    await page.getByRole("button", { name: "Chat actions" }).click();
    await page.getByRole("menuitem", { name: "Chat settings" }).click();
    // Parallelism 0 runs the lead without subagents.
    const parallelism = page.getByRole("textbox", {
      name: "Subagent parallelism",
    });
    await parallelism.fill("0");
    await parallelism.press("Enter");
    await expect(page.locator(".agent-mode-control")).toHaveAttribute(
      "data-concurrency",
      "0",
    );
    await page.keyboard.press("Escape");

    await expect.poll(roles).toEqual(["Lead"]);
    await expect(segments).toHaveCount(0);
    await page
      .getByRole("button", { name: /^Execution settings/ })
      .first()
      .click();
    const picker = page.locator(".mantine-Popover-dropdown.execution-dropdown");
    await expect(
      picker.locator(".mantine-SegmentedControl-label", { hasText: "Lead" }),
    ).toBeVisible();
    await expect(
      picker.locator(".mantine-SegmentedControl-label", {
        hasText: /^Workers$/,
      }),
    ).toHaveCount(0);
  } finally {
    await page.close();
    fixture.kill("SIGTERM");
  }
});
