import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  expect,
  queryFixtureDatabase,
  readTestState,
  rustUiFixture,
  spawnFixture,
  test,
} from "../playwright.ts";
import { fixtureStateDirectory } from "../fixtures/rust-ui-fixture.ts";

for (const action of ["paste", "button"] as const) {
  test(`long text ${action} sends one server asset`, async ({ browser }) => {
    test.setTimeout(90_000);
    const state = await mkdtemp(join(tmpdir(), "text-file-send-"));
    const fixture = spawnFixture(process.execPath, [rustUiFixture, state], {
      stdio: ["pipe", "pipe", "pipe"],
    });
    let log = "";
    fixture.stderr!.on("data", (chunk) => {
      log += String(chunk);
    });
    const context = await browser.newContext();
    try {
      const port = await new Promise<number>((resolve, reject) => {
        fixture.stdout!.once("data", (chunk) =>
          resolve(Number(String(chunk).trim())),
        );
        fixture.once("exit", () => reject(new Error(log)));
      });
      const origin = `http://127.0.0.1:${port}`;
      await readTestState(origin);
      const page = await context.newPage();
      await page.goto(origin);
      await page
        .locator("[data-chat]")
        .filter({ hasText: "Other project" })
        .click();
      const composer = page.locator("#message");
      const text =
        action === "paste" ? "\\".repeat(100_000) : "L".repeat(32_001);
      if (action === "paste") {
        await composer.fill("previous");
        await composer.fill("x".repeat(100_001));
        await expect(composer).toHaveValue("previous");
        await expect(page.getByText(/Draft.*100,000/)).toBeVisible();
        await composer.fill("");
        const accepted = await composer.evaluate((element, value) => {
          const clipboard = new DataTransfer();
          clipboard.setData("text/plain", value);
          return element.dispatchEvent(
            new ClipboardEvent("paste", {
              clipboardData: clipboard,
              bubbles: true,
              cancelable: true,
            }),
          );
        }, text);
        expect(accepted).toBe(true);
        await composer.focus();
        await page.keyboard.insertText(text);
        await expect
          .poll(
            () =>
              queryFixtureDatabase(
                state,
                "SELECT max(length(payload)) AS size FROM sync_documents WHERE scope='drafts'",
              )[0]?.size,
            { timeout: 15_000 },
          )
          .toBeGreaterThan(text.length);
        await page.reload();
        await expect(page.locator("#message")).toHaveValue(text);
      } else await composer.fill(text);
      const sent = page.waitForResponse(
        (response) =>
          new URL(response.url()).pathname === "/api/commands" &&
          response.request().postDataJSON()?.kind === "send",
      );
      if (action === "button")
        await page
          .getByRole("button", { name: "Attach text as a file" })
          .click();
      else await page.locator("#send").click();
      const response = await sent;
      expect(response.ok()).toBe(true);
      const command = response.request().postDataJSON();
      expect(command.body.text).toBe(text);
      expect(command.body.asTextFile).toBe(
        action === "button" ? true : undefined,
      );
      if (action === "button") await page.reload();
      await expect
        .poll(
          () =>
            queryFixtureDatabase(
              state,
              "SELECT count(*) AS count FROM runtime_assets",
            )[0]?.count,
        )
        .toBe(1);
      expect(
        queryFixtureDatabase(
          state,
          "SELECT count(*) AS count FROM runtime_events WHERE id=?",
          command.id,
        )[0]?.count,
      ).toBe(1);
    } finally {
      await context.close();
      if (fixture.exitCode === null && fixture.signalCode === null) {
        const exited = once(fixture, "exit", {
          signal: AbortSignal.timeout(10_000),
        });
        fixture.kill("SIGTERM");
        await exited;
      }
      await rm(state, { recursive: true, force: true });
      await rm(fixtureStateDirectory(state), { recursive: true, force: true });
    }
  });
}
