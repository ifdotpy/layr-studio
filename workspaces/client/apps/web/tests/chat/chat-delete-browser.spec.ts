import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fixtureStateDirectory } from "../fixtures/rust-ui-fixture.ts";
import { spawnFixture, test, expect, rustUiFixture } from "../playwright.ts";

test("chat deletion trusts the server result and settles from SSE", async ({
  page,
}) => {
  test.setTimeout(90_000);
  const state = await mkdtemp(join(tmpdir(), "chat-delete-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, state], {
    stdio: ["pipe", "pipe", "pipe"],
  });
  let log = "";
  fixture.stderr!.on("data", (chunk) => (log += String(chunk)));
  try {
    const port = await new Promise<number>((resolve, reject) => {
      fixture.stdout!.once("data", (chunk) =>
        resolve(Number(String(chunk).trim())),
      );
      fixture.once("exit", () => reject(new Error(log)));
    });
    await page.goto(`http://127.0.0.1:${port}`);
    const chat = page
      .locator("[data-chat]")
      .filter({ hasText: "Release lead" });
    await chat.click();
    const id = await chat.getAttribute("data-chat");
    if (!id) throw new Error("Missing chat id");
    await page.locator("#message").fill("draft to remove");
    const remove = async () => {
      const row = page.locator(`[data-sidebar-item="${id}"]`);
      await row.hover();
      await row
        .getByRole("button", { name: "Actions for Release lead", exact: true })
        .click();
      await page.getByRole("menuitem", { name: "Delete", exact: true }).click();
      await page.locator("[data-delete-chat]").click();
    };
    await page.route(
      "**/api/conversation/delete",
      async (route) => {
        await route.fulfill({ json: { deleted: [id, "new-worker"] } });
      },
      { times: 1 },
    );
    await remove();
    await expect(page.getByText("Delete result unconfirmed")).toHaveCount(0);
    await expect(page.locator("#conversation-title")).not.toHaveText(
      "Release lead",
    );
    await chat.click();
    await expect(page.locator("#message")).toHaveValue("");

    let release!: () => void;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    let sent!: () => void;
    const started = new Promise<void>((resolve) => {
      sent = resolve;
    });
    let forward!: () => void;
    const beforeForward = new Promise<void>((resolve) => {
      forward = resolve;
    });
    await page.route(
      "**/api/conversation/delete",
      async (route) => {
        await beforeForward;
        const response = await route.fetch();
        sent();
        await held;
        await route.fulfill({ response }).catch(() => {});
      },
      { times: 1 },
    );
    const aborted = page.waitForEvent("requestfailed", {
      predicate: (request) =>
        request.url().endsWith("/api/conversation/delete"),
    });
    await remove();
    await expect(page.getByText("Deleting chat…")).toBeVisible();
    forward();
    await started;
    await expect(chat).toHaveCount(0);
    await expect(page.getByText("Deleting chat…")).toHaveCount(0);
    const failure = await aborted;
    expect(failure.failure()?.errorText).toContain("ABORTED");
    release();
  } finally {
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
