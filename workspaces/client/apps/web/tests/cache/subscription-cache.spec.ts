import {
  FrontendLocality,
  SubscriptionCachePolicy,
} from "../../src/generated/api";
import type { AddressInfo } from "node:net";
import { fileURLToPath } from "node:url";
import {
  test,
  expect,
  createFixtureViteServer,
  apiSchemaHandshakeSse,
} from "../playwright.ts";

const workspaceId = "b".repeat(32);
const ref = { kind: "state" };
const sse = (name: string, value: unknown) =>
  `event: ${name}\ndata: ${JSON.stringify(value)}\n\n`;

test("remote cache resumes a delta, corruption snapshots, and local policy writes nothing", async ({
  browser,
}) => {
  let remote = true;
  const received: { since: unknown; reply: string }[] = [];
  const requests: string[] = [];
  const server = await createFixtureViteServer({
    configFile: false,
    root: fileURLToPath(new URL("../../", import.meta.url)),
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "cache-witness",
        configureServer(vite) {
          vite.middlewares.use((request, response, next) => {
            const url = new URL(request.url ?? "/", "http://fixture");
            if (url.pathname === "/cache-witness") {
              response.setHeader("Content-Type", "text/html");
              response.end(
                '<output></output><script type="module" src="/tests/cache/subscription-cache.fixture.ts"></script>',
              );
              return;
            }
            if (url.pathname === "/api/sync/identity") {
              requests.push(url.pathname);
              response.setHeader("Content-Type", "application/json");
              response.end(JSON.stringify({ workspaceId }));
              return;
            }
            if (url.pathname !== "/api/sync/stream") {
              if (url.pathname.startsWith("/api/")) requests.push(url.pathname);
              next();
              return;
            }
            const since: unknown = JSON.parse(
              url.searchParams.get("since") ?? "[]",
            );
            const hasCursor = Array.isArray(since) && since.length > 0;
            const delta = remote && hasCursor;
            received.push({ since, reply: delta ? "delta" : "snapshot" });
            const seq = delta ? 2 : 1;
            response.setHeader("Content-Type", "text/event-stream");
            response.write(
              sse("session", {
                sessionId: "00000000-0000-4000-8000-000000000001",
                workspaceId,
                routes: [],
                locality: remote
                  ? FrontendLocality.remote
                  : FrontendLocality.local,
                cachePolicies: [
                  {
                    resource: ref,
                    policy: remote
                      ? SubscriptionCachePolicy.cache
                      : SubscriptionCachePolicy.noCache,
                  },
                ],
              }),
            );
            response.write(apiSchemaHandshakeSse());
            response.write(
              sse("resources", {
                protocol: 4,
                workspaceId,
                epoch: "fixture",
                revision: seq,
                reason: "initial",
                resources: [ref],
                resourceVersions: [
                  {
                    revision: seq,
                    resource: ref,
                    cursor: { workspaceId, seq },
                    replyKind: delta ? "delta" : "snapshot",
                    entityChanges: {
                      after: delta ? 1 : 0,
                      through: seq,
                      documents: [
                        {
                          op: "upsert",
                          collection: "agentSummary",
                          id: "lead",
                          seq,
                          value: {
                            id: "lead",
                            rootId: "lead",
                            epoch: 1,
                            name: delta ? "delta" : "snapshot",
                            status: "idle",
                          },
                        },
                      ],
                    },
                  },
                ],
              }),
            );
          });
        },
      },
    ],
  });
  await server.listen();
  const address = server.httpServer?.address();
  if (!address || typeof address === "string")
    throw new Error("Fixture server has no port");
  const url = `http://127.0.0.1:${(address satisfies AddressInfo).port}/cache-witness`;
  const context = await browser.newContext();
  const page = await context.newPage();
  try {
    await page.goto(url);
    await expect(page.locator("output")).toHaveText("1");
    await expect
      .poll(() => page.evaluate(() => window.cacheWitness.read()), {
        timeout: 10_000,
      })
      .not.toBeUndefined();
    const coldRequests = [...requests];
    requests.length = 0;
    await page.reload();
    await expect(page.locator("output")).toHaveText("2");
    expect(requests).toEqual(coldRequests);
    requests.length = 0;
    expect(received.at(-1)?.reply).toBe("delta");
    expect(received.at(-1)?.since).toMatchObject([
      { resource: ref, since: { workspaceId, seq: 1 } },
    ]);
    await page.evaluate(() => window.cacheWitness.corrupt());
    await page.reload();
    await expect(page.locator("output")).toHaveText("1");
    expect(received.at(-1)?.reply).toBe("snapshot");
    expect(received.at(-1)?.since).toEqual([]);
    await context.close();
    remote = false;
    const localContext = await browser.newContext();
    const local = await localContext.newPage();
    await local.goto(url);
    await expect(local.locator("output")).toHaveText("1");
    await local.waitForTimeout(5_200);
    expect(
      await local.evaluate(() => window.cacheWitness.read()),
    ).toBeUndefined();
    expect(requests.every((path) => path === "/api/sync/identity")).toBe(true);
    await localContext.close();
  } finally {
    await context.close();
    await server.close();
  }
});
