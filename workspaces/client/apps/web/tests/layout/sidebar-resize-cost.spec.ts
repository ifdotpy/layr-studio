import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";

// A sidebar drag moves the sidebar's own width. It must not rewrite the <html>
// style, which wakes every preference observer and recalculates the style of
// the whole document for each pointer move; the shared width on #root changes
// once, when the drag ends.
test("a sidebar drag rewrites no document style until it ends", async ({
  browser,
}) => {
  test.setTimeout(90_000);
  const root = await mkdtemp(join(tmpdir(), "studio-sidebar-drag-"));
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
    const rail = (await page.locator(".sidebar-resize-rail").boundingBox())!;
    const before = (await page.locator("#sidebar").boundingBox())!.width;
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
    const x = rail.x + rail.width / 2;
    const y = rail.y + 200;
    await page.mouse.move(x, y);
    await page.mouse.down();
    for (let step = 1; step <= 30; step++)
      await page.mouse.move(x + step * 3, y);
    const during = await page.evaluate(
      () =>
        (
          window as typeof window & {
            styleWrites: { html: number; root: number };
          }
        ).styleWrites,
    );
    await page.mouse.up();
    // The sidebar follows the pointer while the shared width stays.
    expect(
      (await page.locator("#sidebar").boundingBox())!.width,
    ).toBeGreaterThan(before + 60);
    expect(during).toEqual({ html: 0, root: 0 });
    // The drag ends by saving the width to #root, where the dock reads it.
    const width = (await page.locator("#sidebar").boundingBox())!.width;
    await expect
      .poll(async () =>
        Math.abs(
          Number.parseFloat(
            await page.evaluate(() =>
              document
                .getElementById("root")!
                .style.getPropertyValue("--studio-sidebar-width"),
            ),
          ) - width,
        ),
      )
      .toBeLessThan(1);
  } finally {
    await page.close();
    fixture.kill("SIGTERM");
  }
});
