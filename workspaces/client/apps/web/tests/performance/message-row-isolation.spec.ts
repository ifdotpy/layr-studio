import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";
import type { FixtureStep } from "../fixtures/fixture-scripts.ts";

const TOKENS = 30;
/** One delta every 50ms, at a fixed transcript size after the initial item. */
const TOKEN_INTERVAL_MS = 50;
type Probe = {
  counts: Record<string, number>;
  rows: Record<string, number>;
  views: number;
};
declare global {
  interface Window {
    transcriptProbe: Probe;
  }
}

test("a streamed reply renders no earlier message per token", async ({
  browser,
}, testInfo) => {
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
            { item: "isolation-stream", delta: "start " },
            { hold: true },
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
    const leadRow = page.locator("[data-chat]", { hasText: "Release lead" });
    const lead = await leadRow.getAttribute("data-chat");
    assert.ok(lead);
    await leadRow.click();
    await page.locator("#message").waitFor();
    await expect(page.locator("[data-message]").nth(1)).toBeVisible();
    const earlier = await page
      .locator("[data-message]")
      .evaluateAll((nodes) =>
        nodes.map((node) => node.getAttribute("data-message")!),
      );
    assert.ok(
      earlier.length >= 2,
      "the fixture has a fixed settled transcript",
    );
    const control = (steps: FixtureStep[]) =>
      fixture.stdin?.write(
        JSON.stringify({ method: "fixture/provider", agent: lead, steps }) +
          "\n",
      );
    await page.locator("#message").fill("Row isolation stream");
    await page
      .getByRole("button", { name: "Send message", exact: true })
      .click();
    const stream = page.locator('[data-message$=":isolation-stream"]');
    await expect(stream).toContainText("start");
    // Let initial membership, send acknowledgement and durable baseline settle.
    await page.waitForTimeout(300);
    const transcriptSize = await page.locator("[data-message]").count();
    const input = await page.locator("#message").elementHandle();
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Performance.enable");
    const before = await cdp.send("Performance.getMetrics");
    await page.evaluate(() => {
      window.transcriptProbe = { counts: {}, rows: {}, views: 0 };
      window.__studioPromptComposerRenderProbe = (kind, id) => {
        const probe = window.transcriptProbe;
        probe.counts[kind] = (probe.counts[kind] || 0) + 1;
        if (kind === "message" && id)
          probe.rows[id] = (probe.rows[id] || 0) + 1;
        if (kind === "message-view") probe.views++;
      };
    });
    for (let index = 0; index < TOKENS; index++) {
      control([
        { item: "isolation-stream", delta: ` token${index}`, append: true },
      ]);
      await page.waitForTimeout(TOKEN_INTERVAL_MS);
    }
    await expect(stream).toContainText(`token${TOKENS - 1}`);
    const counts = await page.evaluate(() => window.transcriptProbe);
    const after = await cdp.send("Performance.getMetrics");
    const metric = (name: string) =>
      (after.metrics.find((row) => row.name === name)?.value || 0) -
      (before.metrics.find((row) => row.name === name)?.value || 0);
    const result = {
      transcriptSize,
      tokens: TOKENS,
      tokensPerSecond: 1000 / TOKEN_INTERVAL_MS,
      ...counts,
      scriptCpuSeconds: metric("ScriptDuration"),
      taskCpuSeconds: metric("TaskDuration"),
      heapDeltaBytes: metric("JSHeapUsedSize"),
    };
    console.log(`Transcript isolation ${JSON.stringify(result)}`);
    await testInfo.attach("transcript-isolation", {
      body: JSON.stringify(result, null, 2),
      contentType: "application/json",
    });
    expect(
      await page
        .locator("#message")
        .evaluate((node, original) => node === original, input),
    ).toBe(true);
    if (process.env.STUDIO_TRANSCRIPT_BASELINE !== "1") {
      expect(counts.counts["turn-history"] || 0).toBe(0);
      expect(counts.counts["history-groups"] || 0).toBe(0);
      expect(counts.counts["prompt-composer"] || 0).toBe(0);
      expect(counts.counts["message"] || 0).toBe(0);
      expect(counts.views).toBe(0);
      expect(counts.counts["message-text"] || 0).toBeGreaterThan(0);
      for (const id of earlier)
        expect(counts.rows[id] || 0, `renders of ${id}`).toBe(0);
    }
    control([
      {
        item: "isolation-stream",
        say:
          "start " +
          Array.from({ length: TOKENS }, (_, index) => ` token${index}`).join(
            "",
          ) +
          " Row isolation finished.",
      },
      { complete: true },
    ]);
    await expect(stream).toContainText("Row isolation finished.");
  } finally {
    await page.close();
    fixture.kill("SIGTERM");
  }
});
