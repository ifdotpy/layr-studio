import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";

// On a touch device the keyboard's Enter adds a new line at every width,
// including a phone on its side and both screens of a dual-screen phone.
// Only the send button sends. A mouse and keyboard keep Enter to send.
for (const device of [
  { name: "touch, wide", width: 1114, height: 720, touch: true, sends: false },
  { name: "touch, narrow", width: 390, height: 844, touch: true, sends: false },
  { name: "mouse, wide", width: 1280, height: 800, touch: false, sends: true },
]) {
  test(`Enter on ${device.name} ${device.sends ? "sends" : "adds a new line"}`, async ({
    browser,
  }) => {
    test.setTimeout(90_000);
    const root = await mkdtemp(join(tmpdir(), "studio-touch-enter-"));
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
      viewport: { width: device.width, height: device.height },
      hasTouch: device.touch,
      isMobile: device.touch,
    });
    const page = await context.newPage();
    try {
      await page.goto(`http://127.0.0.1:${port}`);
      const message = page.locator("#message");
      if (!(await message.isVisible()))
        await page.locator("[data-chat]", { hasText: "Release lead" }).click();
      await message.fill("First line");
      await message.press("Enter");
      if (device.sends) {
        await expect(message).toHaveValue("");
      } else {
        await message.pressSequentially("Second line");
        await expect(message).toHaveValue("First line\nSecond line");
      }
    } finally {
      await context.close();
      fixture.kill("SIGTERM");
    }
  });
}
