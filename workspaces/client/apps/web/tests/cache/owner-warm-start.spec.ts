import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";

// The owner's own browser or installed app opens a second time with its saved
// chats and the open chat's messages before the server answers. The event stream is held back to prove
// that the chats come from the device cache, not from the server.
test("a second open shows saved chats before the server answers", async ({
  browser,
}) => {
  test.setTimeout(90_000);
  const root = await mkdtemp(join(tmpdir(), "studio-warm-start-"));
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
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
  });
  const page = await context.newPage();
  try {
    await page.goto(`http://127.0.0.1:${port}`);
    await page.locator("[data-chat]", { hasText: "Release lead" }).click();
    await page.locator("#message").waitFor();
    // A hidden page writes the subscription cache at once.
    await page.evaluate(() => {
      Object.defineProperty(document, "visibilityState", {
        configurable: true,
        get: () => "hidden",
      });
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await expect
      .poll(() =>
        page.evaluate(async () => {
          const names = (await indexedDB.databases()).map((db) => db.name);
          const name = names.find((value) =>
            value?.startsWith("studio-subscription-cache:"),
          );
          if (!name) return 0;
          const db = await new Promise<IDBDatabase>((resolve, reject) => {
            const request = indexedDB.open(name);
            request.onsuccess = () => resolve(request.result);
            request.onerror = () => reject(request.error);
          });
          const count = await new Promise<number>((resolve) => {
            const request = db
              .transaction("subscriptions")
              .objectStore("subscriptions")
              .count();
            request.onsuccess = () => resolve(request.result);
          });
          db.close();
          return count;
        }),
      )
      .toBeGreaterThan(0);
    await context.route(/\/api\/sync\/stream/, () => {});
    await page.reload({ waitUntil: "domcontentloaded" });
    await expect(
      page.locator("[data-chat]", { hasText: "Release lead" }),
    ).toBeVisible({ timeout: 3_000 });
    await expect(page.getByText("Connecting to LAYR Studio")).toHaveCount(0);
    await expect(page.getByText("Fixture reply")).toBeVisible({
      timeout: 3_000,
    });
  } finally {
    await context.close();
    fixture.kill("SIGTERM");
  }
});
