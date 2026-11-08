import { fixtureOrigin } from "./helpers.ts";
import type {} from "./transient-progress-browser.fixture.tsx";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { test } from "../playwright.ts";

test("transient auth progress has zero idle HTTP reads", async ({
  browser,
}) => {
  const root = resolve(import.meta.dirname, "../..");
  const { createFixtureViteServer: createServer } =
    await import("../playwright.ts");
  const cacheDir = await mkdtemp(join(tmpdir(), "transient-progress-"));
  const server = await createServer({
    configFile: false,
    root,
    cacheDir,
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "transient-progress-fixture",
        configureServer(server) {
          server.middlewares.use("/check", (_req, res) => {
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<div id="root"></div><script type="module" src="/tests/sync/transient-progress-browser.fixture.tsx"></script>',
            );
          });
        },
      },
    ],
  });
  const page = await browser.newPage();
  try {
    await server.listen();
    await page.clock.install();
    await page.goto(fixtureOrigin(server) + "/check");
    const before = await page.evaluate(() => ({
      ...window.reads,
    }));
    await page.clock.fastForward(10_000);
    assert.deepEqual(await page.evaluate(() => window.reads), before);
    assert.deepEqual(before, { facts: 0, auth: 0, jobs: 0 });
    await page.waitForFunction(() =>
      window.selectors.some((resource) => resource.kind === "auth-progress"),
    );
    await page.evaluate(() =>
      window.progress(
        { kind: "auth-progress", requestId: "login-one" },
        { requestId: "login-one", accountKey: "claude-one", status: "ready" },
      ),
    );
    await page.clock.runFor(1);
    await page.waitForFunction(() => window.ready === true);
    console.log(
      JSON.stringify({
        flow: "auth",
        idleSeconds: 10,
        idleReads: 0,
        initialReads: before,
      }),
    );
  } finally {
    await page.close();
    await server.close();
    await rm(cacheDir, { recursive: true, force: true });
  }
});
