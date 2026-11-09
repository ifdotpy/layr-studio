import {
  AgentStatus,
  FrontendLocality,
  SubscriptionCachePolicy,
  type components,
} from "../generated/api.ts";
import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
import type { ResourceChangeEvent } from "./resourceEvents.ts";
import type { Page } from "@playwright/test";
import { initializeDelivery } from "./resourceEntityDelivery.browser.fixture.ts";
import { fixturePort, fixtureSubscriptions } from "./syncBrowserFixture.ts";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  API_SCHEMA_HASH_HEADER,
  apiSchemaHandshakeSse,
  expect,
  protocol3SseEvent,
  readApiSchemaHash,
  test,
} from "../../tests/playwright.ts";

type Entity = components["schemas"]["SyncEntity"];

test("state stream persists records across tabs and recovers through SSE without HTTP data reads", async ({
  browser,
}) => {
  test.setTimeout(60_000);
  const { createFixtureViteServer: createServer } =
    await import("../../tests/playwright.ts");
  const workspaceId = "b".repeat(32);
  const streams = new Set<import("node:http").ServerResponse>();
  const dataReads: string[] = [];
  const rows = new Map<string, Entity>();
  const resource = { kind: "state" } as const;
  let opened = 0;
  let sequence = 0;
  let revision = 0;
  let epoch = "entity-stream-first";
  const put = (id: string, name: string, deleted = false) => {
    const row: Entity = deleted
      ? { op: "delete", collection: "agentSummary", id, seq: ++sequence }
      : {
          op: "upsert",
          collection: "agentSummary",
          id,
          value: agentSummaryFixture(id, {
            name,
            status: AgentStatus.running,
            accountDestinations: [],
          }),
          seq: ++sequence,
        };
    rows.set(row.id, row);
    return row;
  };
  put("lead", "Initial lead");
  put("removed", "Remove me");

  const frame = (
    reason: ResourceChangeEvent["reason"],
    documents: Entity[] = [...rows.values()],
    after?: number,
    reset = false,
  ): ResourceChangeEvent => ({
    protocol: 4,
    workspaceId,
    epoch,
    revision: ++revision,
    reason,
    resources: [resource],
    resourceVersions: [
      {
        revision: sequence,
        resource,
        cursor: { workspaceId, seq: sequence },
        replyKind: after === undefined ? "snapshot" : "delta",
        entityChanges: { after: after ?? 0, through: sequence, documents },
        ...(reset ? { entitySequenceReset: true } : {}),
      },
    ],
  });
  const send = (event: ResourceChangeEvent) => {
    for (const response of streams)
      response.write(protocol3SseEvent("resources", event));
  };
  const cacheDir = await mkdtemp(
    join(tmpdir(), "studio-entity-delivery-vite-"),
  );
  const server = await createServer({
    configFile: false,
    cacheDir,
    root: fileURLToPath(new URL("../..", import.meta.url)),
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "resource-entity-delivery-fixture",
        configureServer(server) {
          server.middlewares.use(async (request, response, next) => {
            const url = new URL(request.url!, "http://localhost");
            if (url.pathname === "/entity-delivery-check") {
              response.setHeader("Content-Type", "text/html");
              response.end(
                "<!doctype html><title>Entity delivery check</title>",
              );
              return;
            }
            if (url.pathname === "/api/sync/stream") {
              response.setHeader("Content-Type", "text/event-stream");
              response.setHeader("Cache-Control", "no-cache");
              response.write(apiSchemaHandshakeSse());
              response.write(
                protocol3SseEvent("session", {
                  sessionId: `entity-session-${opened}`,
                  workspaceId,
                  locality: FrontendLocality.remote,
                  routes: [],
                  cachePolicies: [
                    { resource, policy: SubscriptionCachePolicy.cache },
                  ],
                } satisfies components["schemas"]["SyncSessionEvent"]),
              );
              response.write(
                protocol3SseEvent(
                  "resources",
                  frame(opened++ ? "reconnect" : "initial"),
                ),
              );
              streams.add(response);
              response.on("close", () => streams.delete(response));
              return;
            }
            if (url.pathname === "/api/sync/identity") {
              response.setHeader("Content-Type", "application/json");
              response.setHeader(API_SCHEMA_HASH_HEADER, readApiSchemaHash());
              response.end(
                JSON.stringify({
                  workspaceId,
                  syncProtocol: 2,
                } satisfies components["schemas"]["SyncIdentityResponse"]),
              );
              return;
            }
            if (url.pathname === "/api/sync/subscriptions") {
              const chunks = [];
              for await (const chunk of request) chunks.push(chunk);
              const update = fixtureSubscriptions(
                Buffer.concat(chunks).toString(),
              );
              const ack = {
                gen: update.gen,
                cachePolicies: [
                  { resource, policy: SubscriptionCachePolicy.cache },
                ],
              };
              for (const stream of streams) {
                stream.write(protocol3SseEvent("subscriptions", ack));
                if (update.add.some((entry) => entry.resource.kind === "state"))
                  stream.write(
                    protocol3SseEvent("resources", frame("initial")),
                  );
              }
              response.setHeader("Content-Type", "application/json");
              response.setHeader(API_SCHEMA_HASH_HEADER, readApiSchemaHash());
              response.end(JSON.stringify(ack));
              return;
            }
            if (url.pathname.startsWith("/api/")) {
              dataReads.push(url.pathname);
              response.statusCode = 500;
              response.end("Unexpected HTTP data read");
              return;
            }
            next();
          });
        },
      },
    ],
  });
  const context = await browser.newContext();
  const pages: Page[] = [];
  const errors: string[] = [];
  const openPage = async () => {
    const page = await context.newPage();
    pages.push(page);
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(
      `http://127.0.0.1:${fixturePort(server)}/entity-delivery-check`,
    );
    await page.evaluate(initializeDelivery);
    return page;
  };
  const waitForAgent = (page: Page, id: string, name: string) =>
    page.waitForFunction(
      ({ id, name }) =>
        Array.from(window.nativeState?.agents.values() ?? []).find(
          (agent) => agent.id === id,
        )?.name === name,
      { id, name },
    );
  const waitForCheckpoint = (page: Page, seq: number) =>
    expect.poll(() => page.evaluate(() => window.checkpoint())).toBe(seq);
  const settle = () => new Promise((resolve) => setTimeout(resolve, 150));
  try {
    await server.listen();
    const page = await openPage();
    await waitForAgent(page, "lead", "Initial lead");
    await waitForCheckpoint(page, 2);
    await page.waitForFunction(() => window.connection === "live");
    await settle();
    assert.deepEqual(dataReads, [], "The initial projection comes from SSE");
    const initialOpens = opened;

    send(frame("change", [put("lead", "Stream update")], 2));
    await waitForAgent(page, "lead", "Stream update");
    await waitForCheckpoint(page, 3);
    await settle();
    assert.equal(opened, initialOpens, "A contiguous update keeps the stream");

    send(frame("change", [put("removed", "", true)], 3));
    await page.waitForFunction(
      () =>
        !Array.from(window.nativeState?.agents.values() ?? []).some(
          (agent) => agent.id === "removed",
        ),
    );
    await waitForCheckpoint(page, 4);
    await settle();
    assert.equal(opened, initialOpens, "A tombstone keeps the stream");

    const follower = await openPage();
    await waitForAgent(follower, "lead", "Stream update");
    await follower.waitForFunction(() => window.connection === "live");
    await expect.poll(() => streams.size).toBe(2);
    await settle();
    const beforePeerUpdate = opened;
    send(frame("change", [put("lead", "Shared stream update")], 4));
    await Promise.all(
      pages.map((tab) => waitForAgent(tab, "lead", "Shared stream update")),
    );
    await Promise.all(pages.map((tab) => waitForCheckpoint(tab, 5)));
    await settle();
    assert.equal(
      opened,
      beforePeerUpdate,
      "Both tabs persist the update without replacing their streams",
    );

    put("missing", "Missing range row");
    const gapRow = put("lead", "After gap");
    const beforeGap = opened;
    send(frame("change", [gapRow], 6));
    await Promise.all(
      pages.map((tab) => waitForAgent(tab, "missing", "Missing range row")),
    );
    await waitForAgent(page, "lead", "After gap");
    await waitForCheckpoint(page, 7);
    assert.ok(
      opened > beforeGap,
      "A missing sequence range reopens the SSE stream for a snapshot",
    );

    epoch = "entity-stream-restarted";
    send(frame("change", [put("lead", "New epoch")], 7));
    await Promise.all(
      pages.map((tab) => waitForAgent(tab, "lead", "New epoch")),
    );
    await waitForCheckpoint(page, 8);
    assert.deepEqual(dataReads, [], "Epoch changes stay on SSE");

    put("overflow", "Overflow recovery");
    send(frame("overflow", undefined, undefined, true));
    await Promise.all(
      pages.map((tab) => waitForAgent(tab, "overflow", "Overflow recovery")),
    );
    await waitForCheckpoint(page, 9);
    assert.deepEqual(
      dataReads,
      [],
      "Overflow supplies a complete SSE snapshot",
    );

    const beforeOpen = opened;
    for (const response of streams) response.end();
    put("lead", "After reconnect");
    await expect
      .poll(() => opened, { timeout: 20_000 })
      .toBeGreaterThan(beforeOpen);
    await Promise.all(
      pages.map((tab) => waitForAgent(tab, "lead", "After reconnect")),
    );
    await waitForCheckpoint(page, 10);
    assert.deepEqual(
      dataReads,
      [],
      "Reconnect fills missed changes through SSE",
    );
    assert.deepEqual(errors, []);
    for (const tab of pages)
      assert.deepEqual(await tab.evaluate(() => window.syncErrors), []);
  } finally {
    try {
      await context.close();
      for (const response of streams) response.end();
      await server.close();
    } finally {
      await rm(cacheDir, { recursive: true, force: true });
    }
  }
});
