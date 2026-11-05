import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";

// An open chat shows the event stream state in its activity row. The
// workspace notice above the transcript must not repeat the same message.
test("an open chat shows an event stream failure once", async ({ browser }) => {
  test.setTimeout(90_000);
  const root = await mkdtemp(join(tmpdir(), "studio-transport-once-"));
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
    viewport: { width: 1280, height: 800 },
  });
  try {
    await page.goto(`http://127.0.0.1:${port}`);
    await page.locator("[data-chat]", { hasText: "Release lead" }).click();
    await page.locator("#message").waitFor();
    await page.route("**/api/sync/stream**", (route) => route.abort());
    // The browser network hint closes the open stream; the next one fails.
    await page.evaluate(() => window.dispatchEvent(new Event("offline")));
    await page.evaluate(() => window.dispatchEvent(new Event("online")));
    const failure = page.getByText(/^Connection failed\. Retry scheduled/);
    await expect(failure.first()).toBeVisible({ timeout: 30_000 });
    await expect(failure).toHaveCount(1);
    await expect(page.locator("#error")).toHaveCount(0);
  } finally {
    await page.close();
    fixture.kill("SIGTERM");
  }
});
