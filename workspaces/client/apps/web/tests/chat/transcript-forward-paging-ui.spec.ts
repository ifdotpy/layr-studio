import { cacheFixture } from "../cacheFixture.ts";
import { required } from "../teams/helpers.ts";
import {
  API_SCHEMA_HASH_HEADER,
  entityPullFixtureForRequest,
  handleEntitySyncFixtureRequest,
  readApiSchemaHash,
  readFixtureSyncContract,
  readTestState,
  test,
} from "../playwright.ts";
import type { EntityPullFixtureOptions } from "../playwright.ts";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";

for (const scenario of [
  "paging",
  "bytes",
  "latest",
  "pointer",
  "pointercancel",
] as const)
  test(
    scenario === "bytes"
      ? "byte-bounded older paging preserves a visible user message"
      : scenario === "latest"
        ? "jump to latest reloads an evicted tail"
        : scenario === "pointer" || scenario === "pointercancel"
          ? `paging waits for ${scenario} before restoring`
          : "transcript paging returns from the oldest row to the newest",
    async ({ page, readOnlyRust }) => {
      const byteBounded = scenario === "bytes";
      test.setTimeout(120_000);
      const origin = readOnlyRust.origin;
      const state = await readTestState(origin);
      const syncContract = await readFixtureSyncContract(origin);
      const lead = state.threads.find((agent) => agent.name === "Release lead");
      const other = state.threads.find(
        (agent) => agent.name === "Other project",
      );
      assert.ok(lead && other, "fixture has a managed lead chat");

      const items = Array.from({ length: 600 }, (_, index) => ({
        id: `message-${index}`,
        role: index % 2 ? "assistant" : "user",
        at: index + 1,
        turnId: `turn-${Math.floor(index / 2)}`,
        turnStatus: "completed",
        text:
          byteBounded && index === 240
            ? "large ".repeat(333_100)
            : `Transcript message ${index}`,
        ...(byteBounded && index === 360
          ? { clientMessageId: "visible-client" }
          : {}),
      }));
      const schemaHeaders = {
        [API_SCHEMA_HASH_HEADER]: readApiSchemaHash(),
      };
      const syncFixture = createServer((request, response) => {
        response.setHeader("Access-Control-Allow-Origin", "*");
        if (
          !handleEntitySyncFixtureRequest(request, response, {
            snapshot: state,
            workspaceId: syncContract.identity.workspaceId,
          })
        )
          response.end();
      });
      await new Promise<void>((resolve) =>
        syncFixture.listen(0, "127.0.0.1", resolve),
      );
      const syncOrigin = `http://127.0.0.1:${(syncFixture.address() as AddressInfo).port}`;
      page.on("close", () => syncFixture.closeAllConnections());
      page.on("close", () => syncFixture.close());
      for (const path of ["stream", "subscriptions"]) {
        await page.route(`**/api/sync/${path}*`, (route) => {
          const url = new URL(route.request().url());
          return route.continue({
            url: syncOrigin + url.pathname + url.search,
          });
        });
      }
      const transcriptSyncPulls: { scope: string; after: string }[] = [];
      const transcriptRestReads: string[] = [];
      const pageRequests: {
        id: string | null;
        before: string | null;
        after: string | null;
      }[] = [];
      page.on("request", (request) => {
        if (new URL(request.url()).pathname === "/api/transcript")
          transcriptRestReads.push(request.url());
      });
      await page.addInitScript(({ key, id }) => localStorage.setItem(key, id), {
        key: `codex-desktop-opened:${state.stateDir}`,
        id: cacheFixture(other.id),
      });
      const initialTranscript = {
        agent: {
          ...lead,
          status: "completed",
          inFlight: false,
          turnId: null,
        },
        items: items.slice(-240),
        truncated: true,
        nextCursor: items.at(-240)!.id,
        nextAfterCursor: null,
        historyVersion: "forward-paging-fixture",
        tail: items.at(-1)!.id,
      } satisfies NonNullable<EntityPullFixtureOptions["transcript"]>;
      await page.route("**/api/sync/pull**", async (route) => {
        const url = new URL(route.request().url());
        const scope = url.searchParams.get("scope");
        if (scope !== `transcript:${lead.id}`) return route.continue();
        transcriptSyncPulls.push({
          scope,
          after: url.searchParams.get("after") || "0",
        });
        const projection = entityPullFixtureForRequest(state, url, {
          transcript: initialTranscript,
        });
        await route.fulfill({
          headers: schemaHeaders,
          json: {
            workspaceId: syncContract.identity.workspaceId,
            ...projection,
          },
        });
      });
      let releaseOlder: (() => void) | undefined;
      const olderStarted = Promise.withResolvers<void>();
      let releaseForward: (() => void) | undefined;
      let forwardStarted = Promise.withResolvers<void>();
      await page.route("**/api/transcript/page?*", async (route) => {
        const url = new URL(route.request().url());
        pageRequests.push({
          id: url.searchParams.get("id"),
          before: url.searchParams.get("before"),
          after: url.searchParams.get("after"),
        });
        const before = url.searchParams.get("before");
        const after = url.searchParams.get("after");
        if (
          before &&
          (byteBounded ||
            scenario === "pointer" ||
            scenario === "pointercancel")
        )
          await new Promise<void>((resolve) => {
            releaseOlder = resolve;
            olderStarted.resolve();
          });
        if (after)
          await new Promise<void>((resolve) => {
            releaseForward = resolve;
            forwardStarted.resolve();
          });
        const boundary = before || after;
        const index = items.findIndex((item) => item.id === boundary);
        const selected = before
          ? items.slice(Math.max(0, index - 120), index)
          : after
            ? items.slice(index + 1, index + 121)
            : items.slice(-120);
        const first = items.findIndex((item) => item.id === selected[0]?.id);
        const last = items.findIndex((item) => item.id === selected.at(-1)?.id);
        await route.fulfill({
          headers: schemaHeaders,
          json: {
            items: selected,
            nextCursor: !after && first > 0 ? required(items[first]).id : null,
            nextAfterCursor:
              last >= 0 && last < items.length - 1
                ? required(items[last]).id
                : null,
            historyVersion: "forward-paging-fixture",
          },
        });
      });

      await readOnlyRust.open(page);
      await page.locator(`[data-chat="${lead.id}"]`).click();
      await page.locator('[data-message="message-599"]').waitFor();

      if (scenario === "pointer" || scenario === "pointercancel") {
        const root = page.locator("#messages");
        await root.evaluate((element) => {
          element.scrollTop = 0;
          element.dispatchEvent(
            new WheelEvent("wheel", { deltaY: -1000, bubbles: true }),
          );
        });
        await olderStarted.promise;
        const anchor = await root.evaluate((element) => {
          const bounds = element.getBoundingClientRect();
          const row = Array.from(
            element.querySelectorAll<HTMLElement>("[data-message]"),
          ).find(
            (candidate) =>
              candidate.getBoundingClientRect().bottom > bounds.top &&
              candidate.getBoundingClientRect().top < bounds.bottom,
          )!;
          row.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
          return {
            id: row.dataset.message!,
            offset: row.getBoundingClientRect().top - bounds.top,
            top: element.scrollTop,
          };
        });
        const response = page.waitForResponse((value) =>
          new URL(value.url()).searchParams.has("before"),
        );
        releaseOlder!();
        await response;
        await page.locator('[data-message="message-240"]').waitFor();
        assert.equal(
          await root.evaluate((element) => element.scrollTop),
          anchor.top,
          "viewport stays still while pointer is held",
        );
        await root.evaluate(
          (element, type) =>
            element.dispatchEvent(new PointerEvent(type, { bubbles: true })),
          scenario === "pointercancel" ? "pointercancel" : "pointerup",
        );
        await page.waitForFunction(({ id, offset }) => {
          const root = document.querySelector("#messages")!;
          const row = root.querySelector<HTMLElement>(`[data-message="${id}"]`);
          return (
            !!row &&
            Math.abs(
              row.getBoundingClientRect().top -
                root.getBoundingClientRect().top -
                offset,
            ) < 2
          );
        }, anchor);
        return;
      }
      if (scenario === "latest") {
        const olderResponse = page.waitForResponse((value) =>
          new URL(value.url()).searchParams.has("before"),
        );
        await page.locator("#messages").evaluate((root) => {
          root.scrollTop = 0;
          root.dispatchEvent(
            new WheelEvent("wheel", { deltaY: -1000, bubbles: true }),
          );
        });
        await olderResponse;
        await page
          .getByRole("status")
          .filter({ hasText: "Loading messages" })
          .waitFor({ state: "hidden" });
        const response = page.waitForResponse((value) => {
          const url = new URL(value.url());
          return (
            url.pathname === "/api/transcript/page" &&
            !url.searchParams.has("before") &&
            !url.searchParams.has("after")
          );
        });
        await page.locator("#jump-latest").click();
        await response;
        await page.locator('[data-message="message-599"]').waitFor();
        await page.waitForFunction(() => {
          const root = document.querySelector("#messages")!;
          return root.scrollHeight - root.scrollTop - root.clientHeight < 2;
        });
        return;
      }

      if (byteBounded) {
        const root = page.locator("#messages");
        await root.evaluate((element) => {
          element.scrollTop = 0;
        });
        await page.evaluate(
          () =>
            new Promise((resolve) =>
              requestAnimationFrame(() => requestAnimationFrame(resolve)),
            ),
        );
        const response = page.waitForResponse(
          (value) =>
            value.url().includes("/api/transcript/page?") &&
            new URL(value.url()).searchParams.has("before"),
        );
        await root.evaluate((element) =>
          element.dispatchEvent(
            new WheelEvent("wheel", { deltaY: -1000, bubbles: true }),
          ),
        );
        await olderStarted.promise;
        const anchor = await root.evaluate((element) => {
          const box = element.getBoundingClientRect();
          const visible = Array.from(
            element.querySelectorAll<HTMLElement>("[data-message]"),
          ).find(
            (row) =>
              row.getBoundingClientRect().bottom > box.top &&
              row.getBoundingClientRect().top < box.bottom,
          );
          if (!visible) throw new Error("No visible anchor");
          return {
            id: visible.dataset.message!,
            offset: visible.getBoundingClientRect().top - box.top,
          };
        });
        assert.equal(anchor.id, "message-360");
        releaseOlder!();
        await response;
        await page.waitForFunction(
          ({ id, offset }) => {
            const root = document.querySelector("#messages")!;
            const row = root.querySelector<HTMLElement>(
              `[data-message="${id}"]`,
            );
            return (
              !!row &&
              Math.abs(
                row.getBoundingClientRect().top -
                  root.getBoundingClientRect().top -
                  offset,
              ) < 2
            );
          },
          anchor,
          { timeout: 5000 },
        );
        return;
      }

      for (
        let attempt = 0;
        !(await page.locator('[data-message="message-0"]').count()) &&
        attempt < 10;
        attempt++
      ) {
        const response = page.waitForResponse(
          (value) =>
            value.url().includes("/api/transcript/page?") &&
            new URL(value.url()).searchParams.has("before"),
        );
        await page.locator("#messages").evaluate((root) => {
          root.scrollTop = 0;
          root.dispatchEvent(
            new WheelEvent("wheel", { deltaY: -1000, bubbles: true }),
          );
        });
        const result = await (await response).json();
        if (!result.nextCursor) break;
        await page.evaluate(
          () =>
            new Promise((resolve) =>
              requestAnimationFrame(() => requestAnimationFrame(resolve)),
            ),
        );
      }
      await page.locator("#messages").evaluate((root) => {
        root.dispatchEvent(
          new KeyboardEvent("keydown", { key: "Home", bubbles: true }),
        );
        root.scrollTop = 0;
        root.dispatchEvent(new Event("scroll"));
      });
      await page.locator('[data-message="message-0"]').waitFor();
      let forwardPages = 0;
      for (
        let attempt = 0;
        !(await page.locator('[data-message="message-599"]').count()) &&
        attempt < 10;
        attempt++
      ) {
        const response = page.waitForResponse(
          (value) =>
            value.url().includes("/api/transcript/page?") &&
            new URL(value.url()).searchParams.has("after"),
        );
        await page.locator("#messages").evaluate((root) => {
          root.dispatchEvent(
            new PointerEvent("pointerdown", { bubbles: true }),
          );
          root.scrollTop = root.scrollHeight;
          root.dispatchEvent(new Event("scroll"));
          root.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
        });
        await page.evaluate(
          () =>
            new Promise((resolve) =>
              requestAnimationFrame(() => requestAnimationFrame(resolve)),
            ),
        );
        await forwardStarted.promise;
        await page.locator("#messages").evaluate(
          (root) =>
            new Promise<void>((resolve) => {
              let previous = "";
              let stable = 0;
              const sample = () => {
                const current = `${root.scrollTop}:${root.scrollHeight}`;
                stable = current === previous ? stable + 1 : 0;
                previous = current;
                if (stable >= 3) resolve();
                else requestAnimationFrame(sample);
              };
              requestAnimationFrame(sample);
            }),
        );
        const anchor = await page.locator("#messages").evaluate((root) => {
          const bounds = root.getBoundingClientRect();
          const visible = Array.from(
            root.querySelectorAll<HTMLElement>("[data-message]"),
          ).filter((row) => {
            const box = row.getBoundingClientRect();
            return (
              box.height > 0 &&
              box.bottom > bounds.top &&
              box.top < bounds.bottom
            );
          });
          const content = Array.from(
            root.querySelectorAll<HTMLElement>(
              "[data-message] p, [data-message] pre, [data-message] li",
            ),
          ).find((node) => {
            const box = node.getBoundingClientRect();
            return (
              box.height > 0 &&
              box.bottom > bounds.top &&
              box.top < bounds.bottom
            );
          });
          const row =
            content?.closest<HTMLElement>("[data-message]") || visible[0];
          if (!row)
            throw new Error("no transcript row is visible before paging");
          return {
            id: row.dataset.message,
            offset: row.getBoundingClientRect().top - bounds.top,
          };
        });
        if (!releaseForward)
          throw new Error("Forward request did not start from user input");
        await page
          .locator("#messages")
          .evaluate((root) =>
            root.dispatchEvent(
              new WheelEvent("wheel", { deltaY: 1000, bubbles: true }),
            ),
          );
        releaseForward();
        releaseForward = undefined;
        forwardStarted = Promise.withResolvers<void>();
        const result = await (await response).json();
        await page.evaluate(
          () => new Promise((resolve) => requestAnimationFrame(resolve)),
        );
        await page.waitForFunction(
          ({ id, offset }) => {
            const root = document.querySelector("#messages");
            const row = Array.from(
              root?.querySelectorAll<HTMLElement>("[data-message]") || [],
            ).find((candidate) => candidate.dataset.message === id);
            if (!root || !row) return false;
            const bounds = root.getBoundingClientRect();
            const box = row.getBoundingClientRect();
            const top = box.top - bounds.top;
            return (
              box.bottom >= bounds.top - 32 &&
              box.top < bounds.bottom &&
              Math.abs(top - offset) <= 32
            );
          },
          anchor,
          { timeout: 10_000 },
        );
        forwardPages++;
        if (!result.nextAfterCursor) break;
      }
      await page.locator("#messages").evaluate((root) => {
        root.scrollTop = root.scrollHeight;
        root.dispatchEvent(new Event("scroll"));
      });
      await page.evaluate(
        () =>
          new Promise((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(resolve)),
          ),
      );
      await page.locator('[data-message="message-599"]').waitFor();
      assert.equal(
        await page.locator('[data-message="message-599"]').count(),
        1,
        "newest transcript row is rendered again",
      );
      assert.ok(
        forwardPages > 0,
        "the held window advances through forward pages",
      );
      assert.ok(
        transcriptSyncPulls.some(
          (pull) =>
            pull.scope === `transcript:${lead.id}` && pull.after === "0",
        ),
        "the initial transcript arrives through the active transcript sync pull",
      );
      assert.equal(
        transcriptRestReads.length,
        0,
        "the initial transcript does not fall back to REST",
      );
      assert.equal(
        pageRequests.length,
        pageRequests.filter((request) => request.before !== null).length + 3,
        "all history pages use the page API",
      );
      assert.equal(
        pageRequests.filter((request) => request.before !== null).length >= 3,
        true,
        "older pages use before= requests through the oldest row",
      );
      assert.deepEqual(
        pageRequests
          .filter((request) => request.after !== null)
          .map((request) => request.after),
        ["message-239", "message-359", "message-479"],
        "each forward request starts at the retained newest boundary",
      );
      assert.equal(
        pageRequests.filter((request) => request.after !== null).length,
        3,
        "newer pages use after= requests",
      );
    },
  );
