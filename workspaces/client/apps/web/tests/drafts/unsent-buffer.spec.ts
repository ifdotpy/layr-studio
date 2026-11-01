import type {} from "./unsent-buffer.fixture.tsx";
import type { AddressInfo } from "node:net";
import { fileURLToPath } from "node:url";
import { test, expect, createFixtureViteServer } from "../playwright.ts";

test("unconfirmed draft survives reload and disappears after confirmation", async ({
  page,
}) => {
  const server = await createFixtureViteServer({
    configFile: false,
    root: fileURLToPath(new URL("../../", import.meta.url)),
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "unsent-fixture",
        configureServer(vite) {
          vite.middlewares.use("/unsent-check", (_request, response) => {
            response.setHeader("Content-Type", "text/html");
            response.end(
              '<div id="root"></div><script type="module" src="/tests/drafts/unsent-buffer.fixture.tsx"></script>',
            );
          });
        },
      },
    ],
  });
  await server.listen();
  try {
    const address = server.httpServer?.address();
    if (!address || typeof address === "string")
      throw new Error("Fixture server has no port");
    await page.goto(
      `http://127.0.0.1:${(address satisfies AddressInfo).port}/unsent-check`,
    );
    await expect(page.locator("output")).toHaveText("ready");
    await page.getByLabel("Draft").fill("keep this draft");
    await expect(page.locator("output")).toHaveText("saved");
    await page.reload();
    await expect(page.getByLabel("Draft")).toHaveValue("keep this draft");
    await page.getByRole("button", { name: "Confirm" }).click();
    await expect(page.locator("output")).toHaveText("confirmed");
    await page.reload();
    await expect(page.locator("output")).toHaveText("ready");
    await expect(page.getByLabel("Draft")).toHaveValue("");
    const second = await page.context().newPage();
    await second.goto(page.url());
    await expect(second.locator("output")).toHaveText("ready");
    await page.evaluate(() => window.unsentFixture.put("race", "submitted"));
    await second.evaluate(() => window.unsentFixture.put("race", "newer edit"));
    await page.evaluate(() =>
      window.unsentFixture.deleteIfEqual("race", "submitted"),
    );
    expect(
      await second.evaluate(
        async () =>
          (await window.unsentFixture.loadAll()).find(
            ([key]) => key === "race",
          )?.[1],
      ),
    ).toBe("newer edit");
    await page.getByLabel("Form").fill("submitted form");
    await expect
      .poll(async () =>
        page.evaluate(
          async () =>
            (await window.unsentFormFixture.loadAll()).find(
              ([key]) => key === "review",
            )?.[1],
        ),
      )
      .toBe("submitted form");
    await page.getByRole("button", { name: "Begin request" }).click();
    await second.getByLabel("Form").fill("newer form");
    await expect
      .poll(async () =>
        second.evaluate(
          async () =>
            (await window.unsentFormFixture.loadAll()).find(
              ([key]) => key === "review",
            )?.[1],
        ),
      )
      .toBe("newer form");
    await page.getByRole("button", { name: "Refuse" }).click();
    await expect(page.getByText("Refusal processed")).toBeVisible();
    await expect
      .poll(async () =>
        second.evaluate(
          async () =>
            (await window.unsentFormFixture.loadAll()).find(
              ([key]) => key === "review",
            )?.[1],
        ),
      )
      .toBe("newer form");
    await second.reload();
    await expect(second.getByLabel("Form")).toHaveValue("newer form");
    const recording = await page.evaluate(async () => {
      const store = window.recordingFixture;
      const header = {
        kind: "record" as const,
        value: {
          id: "r",
          chatId: "chat",
          created: 1,
          sampleRate: 16000,
          samples: 1,
          state: "ready" as const,
        },
      };
      const chunk = {
        kind: "chunk" as const,
        id: "r",
        index: 0,
        value: new Blob([new Int16Array([1])]),
      };
      await store.putChunk("record:r", header, 'chunk:["r",0]', chunk);
      const before = (await store.loadAll()).length;
      await store.delete('chunk:["r",0]');
      const after = (await store.loadAll()).length;
      return { before, after };
    });
    expect(recording).toEqual({ before: 2, after: 0 });
    await second.close();
  } finally {
    await server.close();
  }
});

test("removes invalid and unknown-version records at the storage boundary", async ({
  page,
}) => {
  const server = await createFixtureViteServer({
    configFile: false,
    root: fileURLToPath(new URL("../../", import.meta.url)),
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "unsent-decoder-fixture",
        configureServer(vite) {
          vite.middlewares.use("/unsent-check", (_request, response) => {
            response.setHeader("Content-Type", "text/html");
            response.setHeader("Content-Security-Policy", "script-src 'self'");
            response.end(
              '<div id="root"></div><script type="module" src="/tests/drafts/unsent-buffer.fixture.tsx"></script>',
            );
          });
        },
      },
    ],
  });
  await server.listen();
  try {
    const address = server.httpServer?.address();
    if (!address || typeof address === "string")
      throw new Error("Fixture server has no port");
    await page.goto(`http://127.0.0.1:${address.port}/unsent-check`);
    await expect(page.locator("output")).toHaveText("ready");
    const result = await page.evaluate(async () => {
      const buffer = window.unsentFormFixture;
      await buffer.put("valid", "saved");
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("studio-unsent-input");
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
      });
      const key = (id: string) => JSON.stringify(["local", "form", id]);
      await new Promise<void>((resolve, reject) => {
        const tx = db.transaction("input", "readwrite");
        const store = tx.objectStore("input");
        store.put({ broken: true }, key("corrupt"));
        store.put(
          { version: 1, kind: "text", value: "bad", blob: new Blob() },
          key("contradictory"),
        );
        store.put(
          { version: 999, kind: "text", value: "future" },
          key("future"),
        );
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      const values = await buffer.loadAll();
      const remaining = await new Promise<IDBValidKey[]>((resolve, reject) => {
        const tx = db.transaction("input", "readonly");
        const request = tx.objectStore("input").getAllKeys();
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
      });
      db.close();
      return { values, remaining };
    });
    expect(result.values).toContainEqual(["valid", "saved"]);
    for (const id of ["corrupt", "contradictory", "future"])
      expect(result.remaining).not.toContain(
        JSON.stringify(["local", "form", id]),
      );
  } finally {
    await server.close();
  }
});
