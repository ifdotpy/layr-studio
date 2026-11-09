import { FrontendLocality, type components } from "../generated/api.ts";
import type { ResourceChangeEvent } from "./resourceEvents.ts";
import { initializeTranscript } from "./transcriptStream.browser.fixture";
import {
  fixturePort,
  fixtureResources,
  fixtureSubscriptions,
} from "./syncBrowserFixture.ts";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  API_SCHEMA_HASH_HEADER,
  apiSchemaHandshakeSse,
  expect,
  protocol3SseEvent,
  readApiSchemaHash,
  createFixtureViteServer,
  test,
} from "../../tests/playwright.ts";

test("a cached transcript starts instantly and reconnects through SSE without HTTP data reads", async ({
  browser,
}) => {
  test.setTimeout(60_000);
  const workspaceId = "b".repeat(32);
  const resource = { kind: "transcript", agentId: "open" } as const;
  const streams = new Set<import("node:http").ServerResponse>();
  const dataReads: string[] = [];
  const policies = [{ resource, policy: "cache" as const }];
  let sequence = 1;
  let revision = 0;
  let opened = 0;
  let holdBaseline = false;
  let text = "Before streaming";
  const frame = (after?: number): ResourceChangeEvent => ({
    protocol: 4,
    workspaceId,
    epoch: "transcript",
    revision: ++revision,
    reason:
      opened > 1 ? "reconnect" : after === undefined ? "initial" : "change",
    resources: [resource],
    resourceVersions: [
      {
        resource,
        revision: sequence,
        cursor: { workspaceId, seq: sequence },
        replyKind: after === undefined ? "snapshot" : "delta",
        ...(after === undefined
          ? {
              transcriptSnapshot: {
                items: [{ id: "answer", role: "assistant", text }],
                truncated: false,
                nextCursor: null,
              },
            }
          : {
              transcriptChanges: {
                after,
                through: sequence,
                delta: {
                  delta: true,
                  items: [{ id: "answer", role: "assistant", text }],
                  removed: [],
                  truncated: false,
                },
              },
            }),
      },
    ],
  });
  const send = (event: ResourceChangeEvent) => {
    for (const response of streams)
      response.write(protocol3SseEvent("resources", event));
  };
  const server = await createFixtureViteServer({
    configFile: false,
    root: fileURLToPath(new URL("../..", import.meta.url)),
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "transcript-stream-fixture",
        configureServer(server) {
          server.middlewares.use(async (request, response, next) => {
            const url = new URL(request.url!, "http://localhost");
            const json = (value: unknown) => {
              response.setHeader("Content-Type", "application/json");
              response.setHeader(API_SCHEMA_HASH_HEADER, readApiSchemaHash());
              response.end(JSON.stringify(value));
            };
            if (url.pathname === "/transcript-stream-check") {
              response.setHeader("Content-Type", "text/html");
              response.end(
                "<!doctype html><title>Open transcript</title><main></main>",
              );
            } else if (url.pathname === "/api/sync/identity")
              json({ workspaceId });
            else if (url.pathname === "/api/sync/stream") {
              opened++;
              response.setHeader("Content-Type", "text/event-stream");
              response.setHeader("Cache-Control", "no-cache");
              response.write(apiSchemaHandshakeSse());
              response.write(
                protocol3SseEvent("session", {
                  sessionId: "transcript-session",
                  workspaceId,
                  locality: FrontendLocality.remote,
                  cachePolicies: policies,
                  routes: [],
                } satisfies components["schemas"]["SyncSessionEvent"]),
              );
              streams.add(response);
              response.on("close", () => streams.delete(response));
              if (
                !holdBaseline &&
                fixtureResources(
                  url.searchParams.get("resources") || "[]",
                ).some((ref) => ref.kind === "transcript")
              )
                response.write(protocol3SseEvent("resources", frame()));
            } else if (url.pathname === "/api/sync/subscriptions") {
              const chunks = [];
              for await (const chunk of request) chunks.push(chunk);
              const update = fixtureSubscriptions(
                Buffer.concat(chunks).toString(),
              );
              for (const stream of streams) {
                stream.write(
                  protocol3SseEvent("subscriptions", {
                    gen: update.gen,
                    cachePolicies: policies,
                  }),
                );
                if (
                  !holdBaseline &&
                  update.add.some(
                    (entry) => entry.resource.kind === "transcript",
                  )
                )
                  stream.write(protocol3SseEvent("resources", frame()));
              }
              json({ gen: update.gen, cachePolicies: policies });
            } else if (url.pathname.startsWith("/api/")) {
              dataReads.push(url.pathname);
              response.statusCode = 500;
              json({ error: "Unexpected HTTP data read" });
            } else next();
          });
        },
      },
    ],
  });
  const context = await browser.newContext();
  try {
    await server.listen();
    const page = await context.newPage();
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    const url = `http://127.0.0.1:${fixturePort(server)}/transcript-stream-check`;
    await page.goto(url);
    await page.evaluate(initializeTranscript);
    await expect(page.locator("main")).toHaveText(text);
    const after = sequence++;
    text = "Streamed text";
    send(frame(after));
    await expect(page.locator("main")).toHaveText(text);
    await expect
      .poll(() => page.evaluate(() => window.cacheRecord()), {
        timeout: 10_000,
      })
      .toMatchObject({ cursor: { seq: sequence } });
    holdBaseline = true;
    await page.evaluate(() => window.stopTranscript());
    await page.reload();
    await page.evaluate(initializeTranscript);
    await expect(page.locator("main")).toHaveText("Streamed text");
    const beforeReconnect = opened;
    text = "Current after reconnect";
    sequence++;
    holdBaseline = false;
    for (const stream of streams) stream.end();
    await expect(page.locator("main")).toHaveText(text);
    assert.ok(opened > beforeReconnect);
    // A forward gap fails application and reopens from the last applied cursor.
    const gapBase = sequence + 1;
    sequence += 2;
    text = "Gap recovered over SSE";
    send(frame(gapBase));
    await expect(page.locator("main")).toHaveText(text);
    assert.deepEqual(dataReads, []);
    assert.deepEqual(errors, []);
    await page.evaluate(() => window.stopTranscript());
  } finally {
    await context.close();
    for (const response of streams) response.end();
    await server.close();
  }
});
