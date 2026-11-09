import type {} from "./session-poll.fixture.tsx";
import type { AddressInfo } from "node:net";
import type { SessionResponse } from "../playwright.ts";
import { fixtureSnapshot } from "../accounts/fixture-helpers.ts";

import { assert } from "./assertions.ts";
import {
  apiSchemaHeaders,
  handleEntitySyncFixtureRequest,
  legacySnapshotRoute,
  syncIdentityFixture,
  test,
  createFixtureViteServer,
} from "../playwright.ts";
// Replicated state uses the current credential endpoint and reports failures.
import { fileURLToPath } from "node:url";
test("Session Poll Browser", async ({
  browser: _testBrowser,
  context: _testContext,
  page: testPage,
}) => {
  const identity = syncIdentityFixture();
  const snapshot = fixtureSnapshot({
    stateDir: "fixture",
    chats: [],
    threads: [],
    runtime: { tasksHistoryLimit: 1 },
  });
  const server = await createFixtureViteServer({
    configFile: false,
    root: fileURLToPath(new URL("../../", import.meta.url)),
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "session-poll-stream-fixture",
        configureServer(vite) {
          vite.middlewares.use((request, response, next) => {
            if (
              !handleEntitySyncFixtureRequest(request, response, {
                snapshot,
                workspaceId: identity.workspaceId,
              })
            )
              next();
          });
        },
      },
    ],
  });
  await server.listen();
  try {
    const page = testPage;
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    let pulls = 0,
      snapshotReads = 0,
      sessions = 0,
      status = 200,
      token = "first";
    await page.route(legacySnapshotRoute, (route) => {
      snapshotReads++;
      return route.continue();
    });
    await page.route("**/check", (route) =>
      route.fulfill({
        contentType: "text/html",
        body: '<!doctype html><title>Session refresh</title><script type="module" src="/tests/chat/session-poll.fixture.tsx"></script>',
      }),
    );
    await page.route("**/api/session", (route) => {
      sessions++;
      return route.fulfill({
        status,
        headers: apiSchemaHeaders(),
        json:
          status === 200
            ? ({ token } satisfies SessionResponse)
            : { error: "Session unavailable" },
      });
    });
    await page.route("**/api/sync/identity", (route) =>
      route.fulfill({ json: identity, headers: apiSchemaHeaders() }),
    );
    await page.route("**/api/sync/pull?*", (route) => {
      pulls++;
      return route.abort();
    });
    await page.goto(
      `http://127.0.0.1:${(server.httpServer!.address() as AddressInfo).port}/check`,
    );

    await page.waitForFunction(
      () =>
        window.snapshot?.state?.workspaces.values().next().value
          ?.tasksHistoryLimit === 1,
    );
    await page.evaluate(() => window.snapshot.startup.refreshCredentials());
    assert.ok(sessions >= 2);
    assert.equal(
      snapshotReads,
      0,
      "Replicated startup and refresh never download the full state",
    );
    assert.equal(pulls, 0, "Startup receives entity baselines over SSE");
    token = "rotated";
    await page.evaluate(() => window.snapshot.startup.refreshCredentials());
    assert.equal(
      await page.evaluate(() => window.snapshot.api.getToken()),
      "rotated",
    );
    assert.equal(
      await page.evaluate(() => {
        return window.snapshot.state!.workspaces.values().next().value
          ?.tasksHistoryLimit;
      }),
      1,
    );
    for (const failure of [503, 404]) {
      status = failure;
      await page.evaluate(() => window.snapshot.startup.refreshCredentials());
      assert.equal(
        await page.evaluate(() => window.snapshot.startup.error),
        "Session unavailable",
      );
      assert.equal(
        await page.evaluate(() => window.snapshot.api.getToken()),
        "rotated",
      );
      assert.equal(
        snapshotReads,
        0,
        "A missing endpoint or server failure never invokes the old snapshot fallback",
      );
    }
    status = 200;
    token = "recovered";
    await page.evaluate(() => window.snapshot.startup.refreshCredentials());
    assert.equal(
      await page.evaluate(() => window.snapshot.api.getToken()),
      "recovered",
    );
    assert.equal(await page.evaluate(() => window.snapshot.startup.error), "");
    assert.equal(snapshotReads, 0, "Never download the full state");
    assert.equal(snapshotReads, 0, "Never invoke the old snapshot fallback");
    assert.deepEqual(errors, []);
    console.log(
      "session polling PASS: SSE baseline, rotated token, visible error, missing endpoint rejection",
    );
  } finally {
    if (server.httpServer && "closeAllConnections" in server.httpServer)
      server.httpServer.closeAllConnections();
    await server.close();
  }
});
