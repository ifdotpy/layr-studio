import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";
import type { components } from "../../src/generated/api.ts";
import { desktopNotices } from "../../src/components/shell/SupervisorRecoveryNotice/desktopNotices.ts";

// An agent turn changes state many times: it starts, reasons, runs a command,
// reports usage, streams text and ends. None of these steps is the user's
// action, so no control the user aims at may move. The full matrix of layouts,
// devices and keyboards is the manual scripts/layout-shift-matrix.ts.
type Rects = Record<string, [number, number, number, number]>;

for (const device of [
  { name: "desktop", width: 1440, height: 900, touch: false },
  { name: "phone", width: 390, height: 844, touch: true },
]) {
  test(`agent turn moves no composer, header or footer control on ${device.name}`, async ({
    browser,
  }) => {
    test.setTimeout(120_000);
    const root = await mkdtemp(join(tmpdir(), "studio-turn-stability-"));
    const fixture = spawnFixture(process.execPath, [rustUiFixture, root], {
      stdio: ["pipe", "pipe", "pipe"],
      env: {
        ...process.env,
        FIXTURE_WORKER_COUNT: "0",
        FIXTURE_SCRIPT: JSON.stringify([
          { prompt: "Stability turn", steps: [{ hold: true }] },
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
    const desktopResponse = await fetch(`http://127.0.0.1:${port}/api/desktop`);
    assert.ok(desktopResponse.ok);
    const desktop: components["schemas"]["DesktopResponse"] =
      await desktopResponse.json();
    const context = await browser.newContext({
      viewport: { width: device.width, height: device.height },
      hasTouch: device.touch,
      isMobile: device.touch,
    });
    const page = await context.newPage();
    await page.goto(`http://127.0.0.1:${port}`);
    const row = page.locator("[data-chat]", { hasText: "Release lead" });
    const lead = await row.getAttribute("data-chat");
    assert.ok(lead);
    const control = (steps: unknown[]) =>
      fixture.stdin?.write(
        JSON.stringify({ method: "fixture/provider", agent: lead, steps }) +
          "\n",
      );
    await row.click();
    await page.locator("#message").waitFor();
    // Host notices load independently of the chat. Measure the settled idle
    // layout, rather than attributing their initial arrival to the agent turn.
    await expect(page.locator(".supervisor-recovery-notice")).toHaveText(
      desktopNotices(desktop).map((notice) => notice.text),
    );
    const rects = () =>
      page.evaluate(() => {
        const out: Rects = {};
        const selectors = [
          ".workspace-header button",
          "#composer button:not(#stop)",
          "#message",
          ".usage-footer > *",
        ];
        for (const selector of selectors)
          document.querySelectorAll(selector).forEach((node, index) => {
            const r = node.getBoundingClientRect();
            if (!r.width) return;
            const name =
              node.getAttribute("aria-label") ||
              node.id ||
              node.className.toString().split(" ")[0] ||
              String(index);
            out[`${selector} ${name}`] = [r.x, r.y, r.width, r.height].map(
              Math.round,
            ) as Rects[string];
          });
        const slot = document.querySelector("#stop, .composer-stop-slot");
        if (slot) {
          const r = slot.getBoundingClientRect();
          if (r.width)
            out["stop slot"] = [r.x, r.y, r.width, r.height].map(
              Math.round,
            ) as Rects[string];
        }
        return out;
      });
    const settle = () => page.waitForTimeout(600);
    const idle = await rects();
    await page.locator("#message").fill("Stability turn");
    await page.locator("#send").click();
    await page.locator("#stop").waitFor();
    await page.locator("#message").fill("");
    await settle();
    const steps: [string, () => unknown][] = [
      [
        "reasoning",
        () =>
          control([
            {
              notify: {
                method: "item/started",
                params: {
                  item: {
                    id: "r1",
                    type: "reasoning",
                    summary: [],
                    content: [],
                  },
                },
              },
            },
          ]),
      ],
      [
        "command",
        () =>
          control([
            {
              notify: {
                method: "item/started",
                params: {
                  item: {
                    id: "c1",
                    type: "commandExecution",
                    command: "pnpm test",
                    cwd: "/work/project",
                    status: "inProgress",
                    commandActions: [],
                    aggregatedOutput: "",
                  },
                },
              },
            },
          ]),
      ],
      [
        "usage",
        () =>
          control([
            {
              notify: {
                method: "thread/tokenUsage/updated",
                params: {
                  tokenUsage: {
                    total: {
                      totalTokens: 9000,
                      inputTokens: 8000,
                      cachedInputTokens: 0,
                      outputTokens: 1000,
                      reasoningOutputTokens: 0,
                    },
                    last: {
                      totalTokens: 9000,
                      inputTokens: 8000,
                      cachedInputTokens: 0,
                      outputTokens: 1000,
                      reasoningOutputTokens: 0,
                    },
                    modelContextWindow: 200000,
                  },
                },
              },
            },
          ]),
      ],
      [
        "stream",
        () => control([{ item: "s1", delta: "Streaming answer text" }]),
      ],
    ];
    const states: [string, Rects][] = [["working", await rects()]];
    for (const [name, step] of steps) {
      await step();
      await settle();
      states.push([name, await rects()]);
    }
    control([{ say: "Done." }, { complete: true }]);
    await page.locator("#stop").waitFor({ state: "detached" });
    await settle();
    states.push(["ended", await rects()]);
    for (const [name, now] of states)
      for (const [key, rect] of Object.entries(idle)) {
        const after = now[key];
        if (!after) continue;
        assert.ok(
          rect.every((value, index) => Math.abs(value - after[index]!) <= 1),
          `${device.name}: ${key} moved from [${rect}] to [${after}] when the turn reached ${name}`,
        );
      }
    await context.close();
  });
}
