import { tmpdir } from "node:os";
import { join } from "node:path";
import { writeFile } from "node:fs/promises";
import { test, expect } from "../playwright.ts";
import { fixture } from "../servers/multi-server-fixture.ts";
import { SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS } from "../../src/generated/sharedValues.ts";

test("startup cache measurement @live", async ({ browser }) => {
  const server = await fixture("Startup", false, { entityFrames: true });
  const context = await browser.newContext();
  await context.addInitScript(() => {
    const timings: Record<string, number> = {};
    Object.assign(window, { startupTimings: timings });
    const originalFetch = window.fetch.bind(window);
    window.fetch = async (input, init) => {
      const url = new URL(
        input instanceof Request ? input.url : String(input),
        location.href,
      );
      if (url.pathname.startsWith("/api/"))
        timings.firstRequest ??= performance.now();
      if (url.pathname === "/api/sync/stream")
        timings.streamRequest ??= performance.now();
      const response = await originalFetch(input, init);
      if (url.pathname !== "/api/sync/stream" || !response.body)
        return response;
      timings.streamHeaders ??= performance.now();
      const decoder = new TextDecoder();
      let buffer = "";
      const body = response.body.pipeThrough(
        new TransformStream<Uint8Array, Uint8Array>({
          transform(chunk, controller) {
            timings.firstFrame ??= performance.now();
            timings.bytes = (timings.bytes ?? 0) + chunk.byteLength;
            buffer += decoder.decode(chunk, { stream: true });
            let end: number;
            while ((end = buffer.indexOf("\n\n")) !== -1) {
              const frame = buffer.slice(0, end);
              buffer = buffer.slice(end + 2);
              const type = /^event:\s*(.+)$/m.exec(frame)?.[1];
              if (type) {
                timings[`${type}First`] ??= performance.now();
                timings[`${type}Last`] = performance.now();
                timings[`${type}Count`] = (timings[`${type}Count`] ?? 0) + 1;
                if (type === "entity-transfer") {
                  const raw = /^data:\s*(.+)$/m.exec(frame)?.[1];
                  if (raw) {
                    const value = JSON.parse(raw);
                    if (value.resource?.kind === "state" && value.complete)
                      timings.stateCompleteFrame = performance.now();
                  }
                }
              }
            }
            controller.enqueue(chunk);
          },
        }),
      );
      return new Response(body, {
        status: response.status,
        headers: response.headers,
      });
    };
    const observer = new MutationObserver(() => {
      if (
        document.querySelector(".startup, .multi-server-shell, .workspace") &&
        timings.react === undefined
      )
        timings.react = performance.now();
      if (
        document.querySelector("#sidebar") &&
        document.querySelector("#message") &&
        timings.shell === undefined
      )
        requestAnimationFrame(() => {
          timings.shell ??= performance.now();
        });
    });
    observer.observe(document, { subtree: true, childList: true });
    const open = indexedDB.open.bind(indexedDB);
    indexedDB.open = (...args) => {
      timings.idbStart ??= performance.now();
      const request = open(...args);
      request.addEventListener("success", () => {
        timings.idbOpen ??= performance.now();
      });
      return request;
    };
  });
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send("Network.enable");
  let projection = 0;
  cdp.on("Network.eventSourceMessageReceived", (event) => {
    if (event.eventName === "resources") projection = Date.now();
  });
  const results: unknown[] = [];
  try {
    for (const kind of ["cold", "warm", "live-cold", "live-warm"]) {
      const live = kind.startsWith("live");
      const beforePerf = live
        ? await (
            await page.request.get("http://127.0.0.1:4620/api/perf-reporting")
          ).json()
        : undefined;
      projection = 0;
      const started = Date.now();
      await page.goto(live ? "http://127.0.0.1:4620/" : server.origin);
      await expect(page.locator("#message").first()).toBeVisible({
        timeout: 30000,
      });
      await page.waitForTimeout(100);
      const afterPerf = live
        ? await (
            await page.request.get("http://127.0.0.1:4620/api/perf-reporting")
          ).json()
        : undefined;
      const serverDelta =
        beforePerf && afterPerf
          ? Object.fromEntries(
              Object.entries(afterPerf.waits)
                .filter(([key]) => /entity-transfer|sync_stream/.test(key))
                .map(([key, value]) => {
                  const previous = beforePerf.waits[key];
                  if (
                    !value ||
                    typeof value !== "object" ||
                    !("count" in value) ||
                    typeof value.count !== "number" ||
                    !("total_ns" in value) ||
                    typeof value.total_ns !== "number"
                  )
                    throw new Error("Invalid performance aggregate");
                  return [
                    key,
                    {
                      count: value.count - (previous?.count ?? 0),
                      totalMs:
                        (value.total_ns - (previous?.total_ns ?? 0)) / 1e6,
                    },
                  ];
                }),
            )
          : undefined;
      results.push({
        kind,
        serverDelta,
        projectionMs: projection ? projection - started : null,
        ...(await page.evaluate(() => ({
          timings: Reflect.get(window, "startupTimings"),
          resources: performance
            .getEntriesByType("resource")
            .filter((entry) => /api\/|wasm/.test(entry.name))
            .map((entry) => ({
              name: new URL(entry.name).pathname,
              start: entry.startTime,
              duration: entry.duration,
            })),
        }))),
      });
    }
    await writeFile(
      process.env.STUDIO_STARTUP_RESULTS ||
        join(tmpdir(), "startup-measurements.json"),
      JSON.stringify(results, null, 2),
    );
    console.log(JSON.stringify(results));
  } finally {
    await context.close();
    await server.close();
  }
});

test("a direct view paints the cached shell while its initial stream is held; a cold view waits", async ({
  browser,
}) => {
  let revoked = false;
  const server = await fixture("Cached startup", false, {
    handle: ({ url, json }) => {
      if (
        revoked &&
        ["/api/session", "/api/sync/identity", "/api/sync/stream"].includes(
          url.pathname,
        )
      ) {
        json({ error: "Access revoked" }, 401);
        return true;
      }
      return false;
    },
    entityFrames: true,
    cachePolicies: [
      { resource: { kind: "state" }, policy: "cache" },
      {
        resource: {
          kind: "transcript",
          agentId: "overlap",
          tail: SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS,
        },
        policy: "cache",
      },
      {
        resource: { kind: "agentDetail", agentId: "overlap" },
        policy: "cache",
      },
    ],
    transcriptSnapshot: () => ({
      items: [
        {
          id: "saved-message",
          kind: "message",
          role: "assistant",
          text: "Saved startup conversation",
          turnStatus: "completed",
        },
      ],
      truncated: false,
    }),
  });
  // Model identities are qualified by account; provider payload ids stay bare.
  server.snapshot.runtime.modelsByAccount = {
    default: [
      {
        id: "fixture-model",
        model: "fixture-model",
        accountKey: "default",
        catalogPosition: 0,
        provider: "codex",
      },
    ],
  };
  const running = server.snapshot.threads[0];
  if (!running) throw new Error("Missing startup conversation");
  running.inFlight = true;
  running.status = "running";
  const summary = server.snapshot.runtime.agentSummaries?.[0];
  if (!summary) throw new Error("Missing cached agent summary");
  summary.inFlight = true;
  summary.status = "running";
  const context = await browser.newContext();
  await context.addInitScript(() => {
    const times: Record<string, number> = {};
    Object.assign(window, { cachedStartupTimes: times });
    const fetch = window.fetch.bind(window);
    window.fetch = (input, init) => {
      const path = input instanceof Request ? input.url : String(input);
      if (path.includes("/api/")) times.firstRequest ??= performance.now();
      if (path.includes("/api/sync/stream"))
        times.streamRequests = (times.streamRequests ?? 0) + 1;
      return fetch(input, init);
    };
    new MutationObserver(() => {
      if (document.querySelector(".startup, .multi-server-shell, .workspace"))
        times.react ??= performance.now();
      if (
        document.querySelector("#sidebar") &&
        document.body?.textContent?.includes("Saved startup conversation")
      ) {
        requestAnimationFrame(() => {
          times.shellAndConversation ??= performance.now();
        });
      }
    }).observe(document, { childList: true, subtree: true });
  });
  const page = await context.newPage();
  let release = () => {};
  let blocked = false;
  await context.route("**/api/sync/stream?**", async (route) => {
    if (blocked)
      await new Promise<void>((resolve) => {
        release = resolve;
      });
    await route.continue();
  });
  try {
    // The outer shell already has an IndexedDB adapter; seed the same server cache.
    await page.goto(server.origin);
    await expect(page.locator("#message")).toBeVisible();
    await expect(
      page.getByText("Saved startup conversation", { exact: true }),
    ).toBeVisible();
    await expect
      .poll(
        () =>
          page.evaluate(async () => {
            const db = await new Promise<IDBDatabase>((resolve, reject) => {
              const request = indexedDB.open("studio-subscription-cache:local");
              request.onsuccess = () => resolve(request.result);
              request.onerror = () => reject(request.error);
            });
            try {
              if (!db.objectStoreNames.contains("subscriptions")) return 0;
              return await new Promise<number>((resolve) => {
                const request = db
                  .transaction("subscriptions")
                  .objectStore("subscriptions")
                  .count();
                request.onsuccess = () => resolve(request.result);
              });
            } finally {
              db.close();
            }
          }),
        { timeout: 10000 },
      )
      .toBeGreaterThan(2);
    blocked = true;
    await page.goto(
      server.origin + "/?studio-server=local&studio-navigation=classic",
    );
    await expect(page.locator("#message")).toBeVisible({ timeout: 2000 });
    await expect(page.getByLabel("Studio startup")).toHaveCount(0);
    await expect(
      page.getByText("Saved startup conversation", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("status", { name: "Projection sync" }),
    ).toBeVisible();
    await page.waitForFunction(
      () =>
        Reflect.get(window, "cachedStartupTimes").shellAndConversation !==
        undefined,
    );
    await page
      .getByRole("button", { name: "Chat actions", exact: true })
      .click();
    await expect(page.locator('[data-action="stop-team"]')).toBeDisabled();
    await page.keyboard.press("Escape");
    const warmTimes = await page.evaluate(() =>
      Reflect.get(window, "cachedStartupTimes"),
    );
    expect(warmTimes.firstRequest).toBeLessThan(warmTimes.react);
    expect(warmTimes.streamRequests).toBe(1);
    console.info(
      "warm startup shell and conversation",
      JSON.stringify(warmTimes),
    );
    release();
    blocked = false;
    await expect(
      page.getByRole("status", { name: "Projection sync" }),
    ).toHaveCount(0);
    const cold = await browser.newContext();
    const coldPage = await cold.newPage();
    let releaseCold = () => {};
    await cold.route("**/api/sync/stream?**", async (route) => {
      await new Promise<void>((resolve) => {
        releaseCold = resolve;
      });
      await route.continue();
    });
    await coldPage.goto(
      server.origin + "/?studio-server=local&studio-navigation=classic",
    );
    await expect(coldPage.getByLabel("Studio startup")).toBeVisible();
    await expect(coldPage.locator("#message")).toHaveCount(0);
    releaseCold();
    await cold.close();
    revoked = true;
    await page.reload();
    await expect(page.getByLabel("Studio startup")).toBeVisible();
    await expect(page.getByRole("alert")).toContainText("revoked");
    await expect(
      page.getByText("Saved startup conversation", { exact: true }),
    ).toHaveCount(0);
    await page.reload();
    await expect(page.getByRole("alert")).toContainText("revoked");
    await expect(
      page.getByText("Saved startup conversation", { exact: true }),
    ).toHaveCount(0);
  } finally {
    release();
    await context.close();
    await server.close();
  }
});
