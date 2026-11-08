import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, spawnFixture, rustUiFixture } from "../playwright.ts";

test("Team styles retain the app font, viewport bounds and header gap", async ({
  page,
}, testInfo) => {
  test.setTimeout(120_000);
  page.setDefaultTimeout(10_000);
  const evidence = await mkdtemp(join(tmpdir(), "studio-team-visual-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, evidence], {
    stdio: ["ignore", "pipe", "pipe"],
    env: {
      ...process.env,
      FIXTURE_WORKER_COUNT: "24",
      FIXTURE_HOLD: "workers",
    },
  });
  const port = await new Promise<number>((resolve, reject) => {
    fixture.stdout?.once("data", (chunk) =>
      resolve(Number(String(chunk).trim())),
    );
    fixture.once("exit", () =>
      reject(new Error("Fixture exited before readiness")),
    );
  });
  const origin = `http://127.0.0.1:${port}`;
  await page.setViewportSize({ width: 1440, height: 720 });
  await page.goto(origin);
  await expect(page.locator("#message")).toBeVisible();
  await page
    .locator("#sidebar .chat-row")
    .filter({ hasText: "Release lead" })
    .click();
  const recordIcons = async (screen: string, width: number) => {
    const icons = await page.evaluate(() =>
      Object.fromEntries(
        [
          "#sidebar",
          ".workspace-header",
          "#composer",
          "#team",
          "[role=dialog]",
        ].map((selector) => {
          const entries = [
            ...document.querySelectorAll(`${selector} svg`),
          ].flatMap((icon) => {
            const box = icon.getBoundingClientRect();
            return box.width && box.height
              ? [
                  {
                    name:
                      icon.getAttribute("class") ??
                      icon.getAttribute("viewBox"),
                    width: box.width,
                    height: box.height,
                    layoutWidth: getComputedStyle(icon).width,
                    layoutHeight: getComputedStyle(icon).height,
                  },
                ]
              : [];
          });
          return [
            selector,
            [
              ...new Map(
                entries.map((entry) => [JSON.stringify(entry), entry]),
              ).values(),
            ],
          ];
        }),
      ),
    );
    console.log(JSON.stringify({ screen, width, icons }));
  };
  for (const width of [1440, 1024, 390]) {
    await page.setViewportSize({ width, height: 720 });
    await recordIcons("main", width);
    if (width === 1440) {
      await expect(
        page.locator("#team-toggle .team-button-count"),
      ).toBeVisible();
      await expect(page.locator("#team-toggle .team-button-count")).toHaveCSS(
        "margin-inline-start",
        "10px",
      );
      await page.screenshot({ path: testInfo.outputPath("header-1440.png") });
    }
    await page
      .getByRole("button", { name: "Chat actions", exact: true })
      .click();
    await page.getByRole("menuitem", { name: "Team", exact: true }).click();
    const panel = page.locator("#team");
    await expect(panel).toBeVisible();
    await expect(panel.locator(".worker-entry")).toHaveCount(24);
    const geometry = await panel.evaluate((node) => {
      const filter = node.querySelector(".team-summary-filters button");
      const count = document.querySelector("#team-toggle .team-button-count");
      const box = node.getBoundingClientRect();
      return {
        workerHeight: node
          .querySelector(".worker-entry")
          ?.getBoundingClientRect().height,
        maxHeight: getComputedStyle(node).maxHeight,
        stylesheets: [...document.styleSheets].map((sheet) => sheet.href),
        surfaceHeight: getComputedStyle(node).getPropertyValue(
          "--viewport-surface-height",
        ),
        font: filter ? getComputedStyle(filter).fontFamily : null,
        appFont: getComputedStyle(document.body).fontFamily,
        fontSize: filter ? getComputedStyle(filter).fontSize : null,
        expectedSize: getComputedStyle(document.body)
          .getPropertyValue("--studio-text-meta")
          .trim(),
        countGap: count ? getComputedStyle(count).marginInlineStart : null,
        countWidth: count?.getBoundingClientRect().width,
        top: box.top,
        bottom: box.bottom,
        height: box.height,
        viewport: innerHeight,
        overflow: getComputedStyle(node).overflowY,
        scrollHeight: node.scrollHeight,
        clientHeight: node.clientHeight,
      };
    });
    console.log(JSON.stringify({ width, ...geometry }));
    await recordIcons("team", width);
    await page.screenshot({ path: testInfo.outputPath(`team-${width}.png`) });
    expect.soft(geometry.font).toBe(geometry.appFont);
    expect.soft(geometry.fontSize).toBe("12px");
    expect.soft(geometry.workerHeight).toBeLessThan(100);
    await expect(panel.locator(".team-state-icon").first()).toHaveCSS(
      "width",
      "14px",
    );
    await expect(panel.locator(".worker-activity-icon").first()).toHaveCSS(
      "width",
      "12px",
    );
    if (width === 1440)
      expect.soft(parseFloat(geometry.countGap ?? "0")).toBeGreaterThan(0);
    expect.soft(geometry.top).toBeGreaterThanOrEqual(0);
    expect.soft(geometry.bottom).toBeLessThanOrEqual(geometry.viewport);
    expect.soft(geometry.height).toBeLessThanOrEqual(geometry.viewport);
    expect.soft(geometry.overflow).toBe("auto");
    expect.soft(geometry.scrollHeight).toBeGreaterThan(geometry.clientHeight);
    await panel.locator("#team-close").click();
    await expect(panel).toBeHidden();
    if (width === 390) {
      await page.locator("#sidebar-toggle").click();
      await expect(page.locator("#sidebar")).toBeVisible();
      await recordIcons("sidebar", width);
      await page
        .locator("#sidebar .chat-row")
        .filter({ hasText: "Release lead" })
        .click();
    }
    await page
      .getByRole("button", { name: "Chat actions", exact: true })
      .click();
    await page
      .getByRole("menuitem", { name: "Chat settings", exact: true })
      .click();
    const settings = page.getByRole("dialog", {
      name: "Chat settings",
      exact: true,
    });
    await expect(settings).toBeVisible();
    await recordIcons("settings", width);
    await page.screenshot({
      path: testInfo.outputPath(`settings-${width}.png`),
    });
    await settings.getByRole("button", { name: "Close", exact: true }).click();
  }
  console.log(`Team visual evidence: ${evidence}`);
});
