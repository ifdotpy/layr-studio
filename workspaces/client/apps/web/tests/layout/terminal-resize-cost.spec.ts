import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";

// A terminal drag moves the dock's own height. It must not rewrite the <html>
// or #root style, which recalculates the style and layout of the whole page
// for each pointer move; the page height follows once, when the drag ends.
test("a terminal drag rewrites no page style until it ends", async ({
  browser,
}) => {
  test.setTimeout(90_000);
  const root = await mkdtemp(join(tmpdir(), "studio-terminal-drag-"));
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
  const page = await browser.newPage({
    viewport: { width: 1440, height: 900 },
  });
  try {
    await page.goto(`http://127.0.0.1:${port}`);
    await page.locator("[data-chat]", { hasText: "Release lead" }).click();
    await page.locator("#message").waitFor();
    await page.getByRole("button", { name: "Show terminals" }).click();
    const dock = page.locator(".terminal-dock");
    const separator = page.getByRole("separator", {
      name: "Terminal panel height",
    });
    const rail = (await separator.boundingBox())!;
    const before = (await dock.boundingBox())!.height;
    await page.evaluate(() => {
      const counts = { html: 0, root: 0 };
      new MutationObserver(() => counts.html++).observe(
        document.documentElement,
        { attributes: true, attributeFilter: ["style"] },
      );
      new MutationObserver(() => counts.root++).observe(
        document.getElementById("root")!,
        { attributes: true, attributeFilter: ["style"] },
      );
      Object.assign(window, { styleWrites: counts });
    });
    const x = rail.x + 200;
    const y = rail.y + rail.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    for (let step = 1; step <= 30; step++)
      await page.mouse.move(x, y - step * 3);
    const during = await page.evaluate(
      () =>
        (
          window as typeof window & {
            styleWrites: { html: number; root: number };
          }
        ).styleWrites,
    );
    // The dock follows the pointer while the page keeps its height.
    expect((await dock.boundingBox())!.height).toBeGreaterThan(before + 60);
    await page.mouse.up();
    expect(during).toEqual({ html: 0, root: 0 });
    // The drag ends by saving the height to #root, where the page reads it.
    const height = (await dock.boundingBox())!.height;
    await expect
      .poll(() =>
        page.evaluate(() => [
          document
            .getElementById("root")!
            .style.getPropertyValue("--terminal-dock-height"),
          document.documentElement.style.getPropertyValue(
            "--terminal-dock-height",
          ),
        ]),
      )
      .toEqual([`${Math.round(height)}px`, ""]);
  } finally {
    await page.close();
    fixture.kill("SIGTERM");
  }
});
