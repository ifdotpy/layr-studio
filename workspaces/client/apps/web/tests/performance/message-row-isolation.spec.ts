import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";

// A streamed reply changes one message many times. The earlier messages of
// the chat render again only when a value they show changes (the turn starts
// and ends), never once per streamed token.
const TOKENS = 30;
/** Renders a settled message may need while one turn starts and ends. */
const TURN_RENDERS = 4;

test("a streamed reply renders no earlier message per token", async ({
  browser,
}) => {
  test.setTimeout(90_000);
  const root = await mkdtemp(join(tmpdir(), "studio-row-isolation-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, root], {
    stdio: ["pipe", "pipe", "pipe"],
    env: {
      ...process.env,
      FIXTURE_WORKER_COUNT: "0",
      FIXTURE_SCRIPT: JSON.stringify([
        {
          prompt: "Row isolation stream",
          steps: [
            ...Array.from({ length: TOKENS }, (_, index) => ({
              delta: `token${index} `,
            })),
            { say: "Row isolation finished." },
          ],
        },
      ]),
    },
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
    const earlier = await page
      .locator("[data-message]")
      .evaluateAll((nodes) =>
        nodes.map((node) => (node as HTMLElement).dataset.message!),
      );
    assert.ok(earlier.length >= 2, "the fixture chat has earlier messages");
    await page.evaluate(() => {
      const counts: Record<string, number> = {};
      Object.assign(window, { messageRenders: counts });
      window.__studioPromptComposerRenderProbe = (component, id) => {
        if (component === "message" && id) counts[id] = (counts[id] ?? 0) + 1;
      };
    });
    await page.locator("#message").fill("Row isolation stream");
    await page.getByRole("button", { name: "Send message" }).click();
    await expect(page.getByText("Row isolation finished.")).toBeVisible();
    const counts = await page.evaluate(
      () =>
        (window as typeof window & { messageRenders: Record<string, number> })
          .messageRenders,
    );
    for (const id of earlier)
      expect(counts[id] ?? 0, `renders of ${id}`).toBeLessThanOrEqual(
        TURN_RENDERS,
      );
  } finally {
    await page.close();
    fixture.kill("SIGTERM");
  }
});
