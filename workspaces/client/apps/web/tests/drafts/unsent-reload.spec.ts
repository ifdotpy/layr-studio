import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Page, Route } from "@playwright/test";
import { commandOf } from "../messages/send-command.ts";
import {
  test,
  expect,
  readTestState,
  spawnFixture,
  rustUiFixture,
  waitForFixturePort,
} from "../playwright.ts";

function signal() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

// Observe committed IndexedDB transactions without reading the saved record shape.
async function waitForUnsentCount(page: Page, count: number) {
  await page.evaluate(async (expected) => {
    await new Promise<void>((resolve, reject) => {
      let busy = false;
      let changed = false;
      const check = () => {
        if (busy) {
          changed = true;
          return;
        }
        busy = true;
        const open = indexedDB.open("studio-unsent-input");
        open.onerror = () => reject(open.error);
        open.onsuccess = () => {
          const db = open.result;
          const transaction = db.transaction("input", "readonly");
          const request = transaction.objectStore("input").count();
          request.onsuccess = () => {
            busy = false;
            db.close();
            if (request.result === expected) {
              window.removeEventListener("unsent-transaction", check);
              resolve();
            } else if (changed) {
              changed = false;
              check();
            }
          };
          request.onerror = () => reject(request.error);
        };
      };
      window.addEventListener("unsent-transaction", check);
      check();
    });
  }, count);
}

test("unsent input survives reload until each server confirmation", async ({
  page,
}) => {
  test.setTimeout(60_000);
  const root = await mkdtemp(join(tmpdir(), "studio-unsent-reload-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, root], {
    stdio: ["pipe", "pipe", "pipe"],
    env: { ...process.env, FIXTURE_WORKER_COUNT: "1" },
  });
  let log = "";
  fixture.stderr!.on("data", (chunk) => {
    log += chunk;
  });
  const port = await waitForFixturePort(fixture, { diagnostics: () => log });
  const origin = `http://127.0.0.1:${port}`;
  const initial = await readTestState(origin);
  const chatA = initial.threads.find((agent) => agent.name === "Release lead");
  const chatB = initial.threads.find((agent) => agent.name === "Other project");
  expect(chatA).toBeDefined();
  expect(chatB).toBeDefined();
  await page.addInitScript(() => {
    const original = IDBDatabase.prototype.transaction;
    IDBDatabase.prototype.transaction = function (names, mode, options) {
      const transaction = original.call(
        this,
        typeof names === "string" ? names : Array.from(names),
        mode,
        options,
      );
      if (this.name === "studio-unsent-input" && mode === "readwrite")
        transaction.addEventListener("complete", () =>
          window.dispatchEvent(new Event("unsent-transaction")),
        );
      return transaction;
    };
  });
  const requests: { method: string; path: string; type: string }[] = [];
  page.on("request", (request) =>
    requests.push({
      method: request.method(),
      path: new URL(request.url()).pathname,
      type: request.resourceType(),
    }),
  );
  const gates = { send: signal(), draft: signal(), upload: signal() };
  const first = { send: signal(), draft: signal(), upload: signal() };
  const replay = { send: signal(), draft: signal(), upload: signal() };
  const attempts: Record<keyof typeof gates, Buffer[]> = {
    send: [],
    draft: [],
    upload: [],
  };
  let holdDraft = false;
  async function hold(route: Route, kind: keyof typeof gates) {
    attempts[kind].push(route.request().postDataBuffer()!);
    if (attempts[kind].length === 1) {
      first[kind].resolve();
      return;
    }
    replay[kind].resolve();
    await gates[kind].promise;
    await route.continue().catch(() => {});
  }
  await page.route("**/api/commands", (route) =>
    commandOf(route.request(), "send") ? hold(route, "send") : route.continue(),
  );
  await page.route("**/api/sync/drafts", (route) =>
    holdDraft ? hold(route, "draft") : route.continue(),
  );
  await page.route("**/api/assets", (route) => hold(route, "upload"));
  try {
    await page.goto(origin);
    await page.locator(`[data-chat="${chatA!.id}"]`).click();
    const draftSaved = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/sync/drafts" &&
        response.ok(),
    );
    await page.locator("#message").fill("Unconfirmed message");
    await draftSaved;
    const draftCleared = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/sync/drafts" &&
        response.ok(),
    );
    await page.locator("#send").click();
    await first.send.promise;
    await draftCleared;
    await page.locator(`[data-chat="${chatB!.id}"]`).click();
    holdDraft = true;
    await page.locator("#message").fill("Unconfirmed draft");
    await first.draft.promise;
    await page.locator('input[type="file"]').setInputFiles({
      name: "note.txt",
      mimeType: "text/plain",
      buffer: Buffer.from("Unconfirmed upload"),
    });
    await first.upload.promise;
    await waitForUnsentCount(page, 3);
    await page.reload();
    await expect(page.locator("#message")).toHaveValue("Unconfirmed draft");
    await expect(
      page.getByRole("button", { name: "Remove pending note.txt" }),
    ).toBeVisible();
    await Promise.all([
      replay.send.promise,
      replay.draft.promise,
      replay.upload.promise,
    ]);
    expect(JSON.parse(attempts.send[1]!.toString()).id).toBe(
      JSON.parse(attempts.send[0]!.toString()).id,
    );
    expect(attempts.send[1]!.equals(attempts.send[0]!)).toBe(true);
    await page.locator(`[data-chat="${chatA!.id}"]`).click();
    await expect(
      page.locator("#messages").getByText("Unconfirmed message"),
    ).toBeVisible();
    const sendReceipt = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/commands" && response.ok(),
    );
    gates.send.resolve();
    expect((await (await sendReceipt).json()).rows).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ collection: "messageReceipt" }),
      ]),
    );
    await waitForUnsentCount(page, 2);
    const draftReceipt = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/sync/drafts" &&
        response.ok(),
    );
    gates.draft.resolve();
    await draftReceipt;
    await waitForUnsentCount(page, 1);
    const uploadReceipt = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/assets" && response.ok(),
    );
    gates.upload.resolve();
    expect(await (await uploadReceipt).json()).toEqual(
      expect.objectContaining({ name: "note.txt" }),
    );
    await waitForUnsentCount(page, 0);
    await page.reload();
    await page.locator(`[data-chat="${chatB!.id}"]`).click();
    await expect(page.locator("#message")).toHaveValue("Unconfirmed draft");
    await waitForUnsentCount(page, 0);
    await page.locator(`[data-chat="${chatA!.id}"]`).click();
    await expect(
      page.locator("#messages").getByText("Unconfirmed message"),
    ).toBeVisible();
    expect(attempts.send).toHaveLength(2);
    expect(attempts.upload).toHaveLength(2);
    expect(attempts.draft).toHaveLength(2);
    console.log("unsent reload requests", JSON.stringify(requests));
    expect(
      requests.filter(
        ({ method, path, type }) =>
          !(
            (method === "GET" &&
              ((type === "document" && path === "/") ||
                path.startsWith("/assets/") ||
                [
                  "/api/session",
                  "/api/sync/identity",
                  "/api/sync/stream",
                ].includes(path))) ||
            (method === "POST" &&
              [
                "/api/sync/subscriptions",
                "/api/commands",
                "/api/assets",
                "/api/sync/drafts",
                "/api/organization",
              ].includes(path))
          ),
      ),
    ).toEqual([]);
  } finally {
    for (const gate of Object.values(gates)) gate.resolve();
    await page.close();
  }
});

test("confirmed upload stays attached to the draft after reload", async ({
  page,
}) => {
  const root = await mkdtemp(join(tmpdir(), "studio-confirmed-upload-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, root], {
    stdio: ["pipe", "pipe", "pipe"],
    env: { ...process.env, FIXTURE_WORKER_COUNT: "1" },
  });
  let log = "";
  fixture.stderr!.on("data", (chunk) => {
    log += chunk;
  });
  const port = await waitForFixturePort(fixture, { diagnostics: () => log });
  const origin = `http://127.0.0.1:${port}`;
  const initial = await readTestState(origin);
  const chat = initial.threads.find((agent) => agent.name === "Other project");
  expect(chat).toBeDefined();
  await page.goto(origin);
  await page.locator(`[data-chat="${chat!.id}"]`).click();
  const draftSaved = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/api/sync/drafts" && response.ok(),
  );
  await page.locator("#message").fill("Draft with confirmed upload");
  await draftSaved;
  const uploadStarted = signal();
  const releaseUpload = signal();
  await page.route("**/api/assets", async (route) => {
    uploadStarted.resolve();
    await releaseUpload.promise;
    await route.continue();
  });
  const uploaded = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/api/assets" && response.ok(),
  );
  await page.locator('input[type="file"]').setInputFiles({
    name: "confirmed.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("confirmed upload"),
  });
  await uploadStarted.promise;
  await expect(
    page.getByRole("button", { name: "Remove pending confirmed.txt" }),
  ).toBeVisible();
  releaseUpload.resolve();
  expect(await (await uploaded).json()).toEqual(
    expect.objectContaining({ name: "confirmed.txt" }),
  );
  await expect(
    page.getByRole("button", { name: "Remove pending confirmed.txt" }),
  ).toHaveCount(0);
  await page.reload();
  await expect(page.locator("#message")).toHaveValue(
    "Draft with confirmed upload",
  );
  test.fail(
    true,
    "Confirmed upload is not retained in the server draft; work-1793511713761200384-39.",
  );
  await expect(
    page.getByRole("button", { name: "Remove confirmed.txt" }),
  ).toBeVisible();
});
