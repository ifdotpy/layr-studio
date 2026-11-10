import { test, expect } from "@playwright/test";
import { createServer } from "vite";
import { fileURLToPath } from "node:url";

async function fixtureServer() {
  const server = await createServer({
    configFile: false,
    root: fileURLToPath(new URL("../../", import.meta.url)),
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "unsent-close-witness",
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
  return server;
}

// Baseline browser behavior: already green before the native barrier fix.
test("baseline: last unsent text survives immediate tab close and reopen", async ({
  browser,
}) => {
  const server = await fixtureServer();
  const context = await browser.newContext();
  try {
    const address = server.httpServer?.address();
    if (!address || typeof address === "string")
      throw new Error("No fixture port");
    const url = `http://127.0.0.1:${address.port}/unsent-check`;
    const page = await context.newPage();
    await page.goto(url);
    await expect(page.locator("output")).toHaveText("ready");
    await page.getByLabel("Form").fill("last typed text");
    await page.close({ runBeforeUnload: true });
    const reopened = await context.newPage();
    await reopened.goto(url);
    await expect(reopened.getByLabel("Form")).toHaveValue("last typed text");
  } finally {
    await context.close();
    await server.close();
  }
});

declare global {
  interface Window {
    unsentCloseRequest: (event: { payload: { id: number } }) => Promise<void>;
    unsentCloseResponses: Record<string, unknown>[];
    unsentCloseReady: boolean;
    releaseUnsentTransaction: () => void;
  }
}

test("native close drains queued composer edits and restores the last text", async ({
  browser,
}) => {
  const server = await fixtureServer();
  const context = await browser.newContext();
  await context.addInitScript(() => {
    window.unsentCloseResponses = [];
    window.unsentCloseReady = false;
    Object.defineProperty(globalThis, "__TAURI__", {
      value: {
        core: {
          invoke: async (command: string, args: Record<string, unknown>) => {
            if (command === "close_response")
              window.unsentCloseResponses.push(args);
            if (command === "close_ready") window.unsentCloseReady = true;
          },
        },
        event: {
          listen: async (
            name: string,
            callback: typeof window.unsentCloseRequest,
          ) => {
            if (name === "studio-close-request")
              window.unsentCloseRequest = callback;
            return () => {};
          },
        },
      },
    });
  });
  try {
    const address = server.httpServer?.address();
    if (!address || typeof address === "string")
      throw new Error("No fixture port");
    const url = `http://127.0.0.1:${address.port}/unsent-check`;
    const page = await context.newPage();
    await page.goto(url);
    await expect(page.locator("output")).toHaveText("ready");
    await expect
      .poll(() => page.evaluate(() => window.unsentCloseReady))
      .toBe(true);
    const blocker = await context.newPage();
    await blocker.goto(url);
    await expect(blocker.locator("output")).toHaveText("ready");
    // Another view owns an active transaction. The writer must wait for the
    // real IndexedDB transaction, rather than treating enqueue as persistence.
    await blocker.evaluate(
      () =>
        new Promise<void>((resolve, reject) => {
          const request = indexedDB.open("studio-unsent-input");
          request.onerror = () => reject(request.error);
          request.onsuccess = () => {
            const db = request.result;
            const tx = db.transaction("input", "readwrite");
            let released = false;
            window.releaseUnsentTransaction = () => {
              released = true;
            };
            const keepOpen = () => {
              if (!released)
                tx.objectStore("input").get("hold").onsuccess = keepOpen;
            };
            tx.oncomplete = () => db.close();
            keepOpen();
            resolve();
          };
        }),
    );
    await page.getByLabel("Form").fill("native last typed text");
    await page.getByLabel("Draft", { exact: true }).fill("composer edit A");
    // The wrapping label's text query changes after the textarea contains text.
    // Its textbox accessible name stays stable for successive local edits.
    await page
      .getByRole("textbox", { name: "Draft", exact: true })
      .fill("composer edit B");
    await expect(
      page.getByRole("textbox", { name: "Draft", exact: true }),
    ).toHaveValue("composer edit B");
    await page.evaluate(() => {
      void window.unsentCloseRequest({ payload: { id: 1 } });
    });
    expect(await page.evaluate(() => window.unsentCloseResponses)).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.inert)).toBe(
      true,
    );
    await blocker.evaluate(() => window.releaseUnsentTransaction());
    await expect
      .poll(() => page.evaluate(() => window.unsentCloseResponses))
      .toEqual([{ id: 1, ready: true }]);
    await page.close();
    const reopened = await context.newPage();
    await reopened.goto(url);
    await expect(reopened.getByLabel("Form")).toHaveValue(
      "native last typed text",
    );
    await expect(
      reopened.getByRole("textbox", { name: "Draft", exact: true }),
    ).toHaveValue("composer edit B");
  } finally {
    await context.close();
    await server.close();
  }
});
