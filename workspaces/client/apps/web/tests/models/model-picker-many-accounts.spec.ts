import assert from "node:assert/strict";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Page } from "@playwright/test";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";

// The model picker keeps one height for every role. With many accounts the
// controls need more than that height: the model list may shrink to a few
// rows, and then the picker body scrolls. No control may be squeezed or cut.
const ACCOUNTS = 6;
const ROLES = ["Lead", "Workers", "Review"];

async function squeezed(page: Page, picker: string) {
  return page.evaluate((picker) => {
    const box = [...document.querySelectorAll<HTMLElement>(picker)]
      .filter((node) => node.getClientRects().length)
      .pop();
    if (!box) return ["no picker"];
    const problems: string[] = [];
    // A segmented control or a tile shorter than its content is squeezed.
    for (const node of box.querySelectorAll<HTMLElement>(
      ".mantine-SegmentedControl-root, .setup-account, .mantine-Button-root",
    ))
      if (
        node.getClientRects().length &&
        node.scrollHeight > node.clientHeight + 1
      )
        problems.push(
          `${node.className.split(" ")[0]} ${node.clientHeight}/${node.scrollHeight}`,
        );
    const list = box.querySelector<HTMLElement>(".setup-models");
    const row = list?.firstElementChild?.getBoundingClientRect().height ?? 0;
    if (
      list &&
      list.scrollHeight > list.clientHeight + 1 &&
      list.clientHeight < row * 3 - 1
    )
      problems.push(`model list ${list.clientHeight}px for rows of ${row}px`);
    return problems;
  }, picker);
}

for (const device of [
  { name: "desktop", width: 1440, height: 900, touch: false },
  { name: "short laptop", width: 1280, height: 640, touch: false },
]) {
  test(`model picker with many accounts squeezes no control on ${device.name}`, async ({
    browser,
  }) => {
    test.setTimeout(120_000);
    const root = await mkdtemp(join(tmpdir(), "studio-picker-accounts-"));
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
    for (let index = 1; index <= ACCOUNTS; index++) {
      const home = join(root, `account-${index}`);
      await mkdir(home);
      await writeFile(
        join(home, "auth.json"),
        JSON.stringify({ OPENAI_API_KEY: "fixture-key-never-sent" }),
      );
      const response = await fetch(`${origin}/api/accounts/register`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Canvas-Token": token,
        },
        body: JSON.stringify({ home }),
      });
      assert.ok(response.ok, await response.text());
    }
    const context = await browser.newContext({
      viewport: { width: device.width, height: device.height },
    });
    const page = await context.newPage();
    try {
      await page.goto(origin);
      await page.locator("[data-chat]", { hasText: "Release lead" }).click();
      await page.locator("#message").waitFor();
      const roles = async (picker: string) => {
        for (const role of ROLES) {
          await page
            .locator(`${picker} .mantine-SegmentedControl-label`)
            .filter({ hasText: new RegExp(`^${role}$`) })
            .click();
          await expect
            .poll(() => squeezed(page, picker), { message: role })
            .toEqual([]);
        }
      };
      await page
        .getByRole("button", { name: /^Execution settings/ })
        .first()
        .click();
      await roles(".mantine-Popover-dropdown.execution-dropdown");
      // A tile with a limit bar keeps its name clear of the bar and starts
      // the name, the bar and the model names at one inset. The fixture
      // accounts report no limits, so the tile gets the bar markup that
      // AccountRemainingBar renders.
      const tile = await page.evaluate(() => {
        const tile = document.querySelector<HTMLElement>(
          ".execution-dropdown .setup-account:not(.setup-account-automatic)",
        )!;
        const bar = document.createElement("span");
        bar.className = "setup-limit";
        bar.append(document.createElement("span"));
        tile.querySelector(".mantine-Button-inner")!.after(bar);
        const box = tile.getBoundingClientRect();
        const name = tile
          .querySelector(".setup-account-name")!
          .getBoundingClientRect();
        const limit = bar.getBoundingClientRect();
        const model = document
          .querySelector(".execution-dropdown .setup-models button")!
          .getBoundingClientRect();
        return {
          gap: limit.top - name.bottom,
          above: name.top - box.top,
          below: box.bottom - limit.bottom,
          insets: [name.left, limit.left].map((left) =>
            Math.round(left - box.left),
          ),
          modelInset: Math.round(
            document
              .querySelector(
                ".execution-dropdown .setup-models button .mantine-Button-label",
              )!
              .getBoundingClientRect().left - model.left,
          ),
        };
      });
      expect(tile.gap).toBeGreaterThanOrEqual(3);
      expect(Math.abs(tile.above - tile.below)).toBeLessThanOrEqual(1);
      expect(tile.insets).toEqual([tile.modelInset, tile.modelInset]);
      await page.keyboard.press("Escape");
      await page
        .getByRole("button", { name: "New chat", exact: true })
        .first()
        .click();
      await roles(".new-chat-model-settings");
      // The Start button stays reachable: inside the window or by scrolling.
      const start = page.getByRole("button", { name: /^Start/ }).last();
      await start.scrollIntoViewIfNeeded();
      await expect(start).toBeInViewport();
    } finally {
      await context.close();
      fixture.kill("SIGTERM");
    }
  });
}
