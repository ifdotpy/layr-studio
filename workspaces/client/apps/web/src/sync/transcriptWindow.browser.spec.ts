import type { LegacySyncIdentityResponse } from "./syncBrowserFixture.ts";
import { SubscriptionReplyKind, type components } from "../generated/api.ts";
import type { ResourceChangeEvent, ResourceRef } from "./resourceEvents.ts";
import {
  fixturePort,
  type LegacySyncPullResponse,
  fixtureSubscriptions,
  fixtureResources,
} from "./syncBrowserFixture.ts";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  API_SCHEMA_HASH_HEADER,
  apiSchemaHandshakeSse,
  createFixtureViteServer,
  expect,
  protocol3SseEvent,
  readApiSchemaHash,
  syncSessionSse,
  test,
} from "../../tests/playwright.ts";
import {
  SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS,
  SYNC_STREAM_EVENT_MAX_BYTES,
} from "../generated/sharedValues.ts";

for (const equalSequenceReceipt of [
  SubscriptionReplyKind.delta,
  SubscriptionReplyKind.snapshot,
]) {
  test(`transcript window applies equal-sequence ${equalSequenceReceipt} receipts and explicit history paging`, async ({
    browser,
  }) => {
    test.setTimeout(60_000);
    const workspaceId = "7".repeat(32);
    const all = Array.from({ length: 120 }, (_, index) => ({
      id: `open:item-${String(index).padStart(3, "0")}`,
      role: "assistant",
      text: `History item ${index}`,
      at: index,
    }));
    const pulls: (string | null)[] = [];
    const pages: Record<string, string>[] = [];
    const streams = new Set<import("node:http").ServerResponse>();
    let revision = 0;
    let transcriptSequence = 1;
    let deltaBaseline = false;
    let unchangedBaseline = false;
    const snapshot = {
      items: all.slice(-SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
      truncated: true,
      nextCursor: all.at(-SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS)!.id,
      nextAfterCursor: null,
      historyVersion: "window-fixture",
    } satisfies components["schemas"]["TranscriptPageResponse"];
    const baseline = (
      response: import("node:http").ServerResponse,
      resources: ResourceRef[],
    ) => {
      const refs = resources.filter((ref) => ref.kind === "transcript");
      response.write(
        protocol3SseEvent("resources", {
          protocol: 4,
          workspaceId,
          epoch: "window",
          revision: unchangedBaseline ? revision : ++revision,
          reason: "initial",
          resources: refs,
          resourceVersions: refs.map(() => ({
            revision: transcriptSequence,
            cursor: { workspaceId, seq: transcriptSequence },
            ...(deltaBaseline
              ? {
                  replyKind: SubscriptionReplyKind.delta,
                  ...(unchangedBaseline
                    ? {}
                    : {
                        transcriptChanges: {
                          after: transcriptSequence - 1,
                          through: transcriptSequence,
                          delta: {
                            delta: true,
                            items: [all.at(-1)!],
                            removed: [],
                            truncated: true,
                            historyVersion: snapshot.historyVersion,
                          },
                        },
                      }),
                }
              : {
                  replyKind: SubscriptionReplyKind.snapshot,
                  transcriptSnapshot: snapshot,
                }),
          })),
        } satisfies ResourceChangeEvent),
      );
    };
    const server = await createFixtureViteServer({
      configFile: false,
      root: fileURLToPath(new URL("../..", import.meta.url)),
      server: { host: "127.0.0.1", port: 0 },
      plugins: [
        {
          name: "transcript-window-fixture",
          configureServer(server) {
            server.middlewares.use(async (request, response, next) => {
              const url = new URL(request.url!, "http://localhost");
              const json = (value: unknown) => {
                response.setHeader("Content-Type", "application/json");
                response.setHeader(API_SCHEMA_HASH_HEADER, readApiSchemaHash());
                response.end(JSON.stringify(value));
              };
              if (url.pathname === "/window-check") {
                response.setHeader("Content-Type", "text/html");
                response.end(
                  "<!doctype html><title>History window</title><main></main>",
                );
              } else if (url.pathname === "/api/sync/identity")
                json({
                  workspaceId,
                } satisfies LegacySyncIdentityResponse);
              else if (url.pathname === "/api/sync/stream") {
                response.setHeader("Content-Type", "text/event-stream");
                response.setHeader("Cache-Control", "no-cache");
                response.write(syncSessionSse("window-session", workspaceId));
                response.write(apiSchemaHandshakeSse());
                baseline(
                  response,
                  fixtureResources(url.searchParams.get("resources") || "[]"),
                );
                streams.add(response);
                response.on("close", () => streams.delete(response));
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
                      cachePolicies: [],
                    } satisfies components["schemas"]["SyncSubscriptionAck"]),
                  );
                  baseline(
                    stream,
                    update.add.map((entry) => entry.resource),
                  );
                }
                json({
                  gen: update.gen,
                  cachePolicies: [],
                } satisfies components["schemas"]["SyncSubscriptionAck"]);
              } else if (url.pathname === "/api/sync/pull") {
                pulls.push(url.searchParams.get("scope"));
                json({
                  workspaceId,
                  documents: [
                    {
                      id: "transcript:open",
                      payload: JSON.stringify(snapshot),
                      seq: transcriptSequence,
                      _deleted: false,
                    },
                  ],
                  checkpoint: { seq: transcriptSequence },
                } satisfies LegacySyncPullResponse);
              } else if (url.pathname === "/api/transcript/page") {
                pages.push(Object.fromEntries(url.searchParams));
                const before = all.findIndex(
                  (item) => item.id === url.searchParams.get("before"),
                );
                const limit = Number(url.searchParams.get("limit"));
                const start = Math.max(0, before - limit);
                assert.ok(all[start]);
                json({
                  items: all.slice(start, before),
                  truncated: start > 0,
                  nextCursor: start ? all[start].id : null,
                  nextAfterCursor: all[before - 1]?.id || null,
                  historyVersion: "window-fixture",
                } satisfies components["schemas"]["TranscriptPageResponse"]);
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
      await page.goto(`http://127.0.0.1:${fixturePort(server)}/window-check`);
      await page.evaluate((workspaceId) => {
        document.documentElement.dataset.workspaceId = workspaceId;
      }, workspaceId);
      await page.addScriptTag({
        type: "module",
        url: "/src/sync/transcriptWindow.browser.fixture.tsx",
      });
      await page.waitForFunction(() => !!window.historyRoot);
      await expect(page.locator("output")).toHaveAttribute(
        "data-count",
        String(SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
      );
      assert.deepEqual(
        pulls,
        [],
        "a normal initial stream window requires zero pulls",
      );
      await page.locator("#history-scroll").evaluate((root) => {
        root.dispatchEvent(
          new WheelEvent("wheel", { deltaY: -2000, bubbles: true }),
        );
        root.scrollTop = 0;
        root.dispatchEvent(new Event("scroll", { bubbles: true }));
      });
      await expect(page.locator("output")).toHaveAttribute(
        "data-count",
        String(2 * SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
      );
      assert.equal(
        pages.length,
        1,
        "one user action issues exactly one page read",
      );
      assert.ok(pages[0]);
      assert.equal(Number(pages[0].limit), SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS);
      assert.equal(pages[0].before, snapshot.nextCursor);
      assert.deepEqual(
        (await page.locator("output").textContent())!.split(","),
        all
          .slice(-2 * SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS)
          .map((item) => item.id),
      );
      assert.deepEqual(pulls, []);
      if (equalSequenceReceipt === SubscriptionReplyKind.delta) {
        const receipts = await page.evaluate(
          () => window.historyReceipts.length,
        );
        deltaBaseline = true;
        unchangedBaseline = true;
        for (const stream of streams) stream.end();
        await expect
          .poll(() => page.evaluate(() => window.historyReceipts.length))
          .toBeGreaterThan(receipts);
        expect(await page.evaluate(() => window.historyReceipts.at(-1))).toBe(
          SubscriptionReplyKind.delta,
        );
        await expect(page.locator("output")).toHaveAttribute(
          "data-count",
          String(2 * SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
        );
        unchangedBaseline = false;
      }
      const oldTailText = all.at(-1)!.text;
      all.at(-1)!.text =
        "Recovered oversized transcript " +
        "界".repeat(Math.ceil(SYNC_STREAM_EVENT_MAX_BYTES / 3) + 1);
      const deltaAfter = transcriptSequence++;
      const resource = {
        kind: "transcript",
        agentId: "open",
        tail: SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS,
      } as const;
      const cursor = { workspaceId, seq: transcriptSequence };
      const entry: components["schemas"]["ResourceRevisionEntry"] = {
        resource,
        revision: transcriptSequence,
        cursor,
        replyKind: SubscriptionReplyKind.delta,
        transcriptChanges: {
          after: deltaAfter,
          through: transcriptSequence,
          delta: {
            delta: true,
            items: [all.at(-1)!],
            removed: [],
            truncated: true,
            historyVersion: snapshot.historyVersion,
          },
        },
      };
      const encoded = JSON.stringify(entry);
      const fragmentChars = 30_000;
      const partCount = Math.ceil(encoded.length / fragmentChars);
      const frames = Array.from({ length: partCount }, (_, partIndex) =>
        protocol3SseEvent("entity-transfer", {
          protocol: 4,
          payloadKind: "transcript",
          workspaceId,
          epoch: "window",
          revision: revision + 1,
          reason: "change",
          resource,
          cursor,
          replyKind: SubscriptionReplyKind.delta,
          transferId: "oversized-window",
          pageIndex: 0,
          partIndex,
          partCount,
          complete: partIndex + 1 === partCount,
          data: encoded.slice(
            partIndex * fragmentChars,
            (partIndex + 1) * fragmentChars,
          ),
        } satisfies components["schemas"]["ResourceEntityTransferEvent"]),
      );
      revision++;
      for (const stream of streams)
        for (const frame of frames.slice(0, -1)) stream.write(frame);
      await page.evaluate(() => new Promise(requestAnimationFrame));
      const last = page.locator('[data-message="open:item-119"]');
      await expect(last).toHaveText(oldTailText);
      for (const stream of streams) stream.write(frames.at(-1)!);
      await expect
        .poll(() => last.evaluate((node) => node.textContent!.length))
        .toBe(all.at(-1)!.text.length);
      await expect(page.locator("output")).toHaveAttribute(
        "data-count",
        String(2 * SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
      );
      assert.deepEqual(
        pulls,
        [],
        "a completed oversized interval uses only SSE fragments",
      );
      deltaBaseline = true;
      all.at(-1)!.text = "After reconnect";
      transcriptSequence++;
      for (const stream of streams) stream.end();
      await expect(last).toHaveText("After reconnect");
      await expect(page.locator("output")).toHaveAttribute(
        "data-count",
        String(2 * SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
      );
      assert.equal(
        pages.length,
        1,
        "a reconnect delta keeps loaded older history without reading it again",
      );
      deltaBaseline = false;
      const receipts = await page.evaluate(() => window.historyReceipts.length);
      if (equalSequenceReceipt !== SubscriptionReplyKind.snapshot) {
        all.at(-1)!.text = "After reset";
        transcriptSequence++;
      }
      for (const stream of streams) stream.end();
      await expect
        .poll(() => page.evaluate(() => window.historyReceipts.length))
        .toBeGreaterThan(receipts);
      expect(await page.evaluate(() => window.historyReceipts.at(-1))).toBe(
        SubscriptionReplyKind.snapshot,
      );
      await expect(last).toHaveText(all.at(-1)!.text);
      await expect(page.locator("output")).toHaveAttribute(
        "data-count",
        String(SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
      );
      assert.equal(
        pages.length,
        1,
        "a reset snapshot does not automatically refetch old history",
      );
      await page.getByRole("button", { name: "Older history" }).click();
      await expect.poll(() => pages.length).toBe(2);
      await expect(page.locator("output")).toHaveAttribute(
        "data-count",
        String(2 * SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS),
      );
      assert.equal(
        pages.length,
        2,
        "one explicit page restores history after the reset",
      );
      assert.deepEqual(pulls, []);
      assert.deepEqual(errors, []);
      await page.evaluate(() => window.historyRoot.unmount());
    } finally {
      await context.close();
      for (const response of streams) response.end();
      await server.close();
    }
  });
}
