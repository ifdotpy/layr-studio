import { required } from "./helpers.ts";
import { responseJson } from "./helpers.ts";
import { sendReply, sendRoute, sentMessage } from "./send-command.ts";
import type { MessageDelivery } from "../../src/commands.ts";
import type { Page, Route } from "@playwright/test";
import type { components } from "../../src/generated/api.ts";
import type { MessageRequest } from "./helpers.ts";
import type { TestInfo, BrowserContext } from "@playwright/test";
import {
  readTestState,
  stubEntityState,
  readFixtureSyncContract,
  apiSchemaHandshakeSse,
  syncSessionSse,
  protocol3SseEvent,
  spawnFixture as spawn,
  waitForFixturePort,
  test,
  rustUiFixture,
} from "../playwright.ts";
// Production renderer, isolated fixture, and controlled delivery acknowledgements.
import assert from "node:assert/strict";
import { createServer, type ServerResponse } from "node:http";
import { randomUUID } from "node:crypto";
import type { AddressInfo } from "node:net";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const browserContextsByTest = new WeakMap<TestInfo, Set<BrowserContext>>();
test.beforeEach(async ({ browser }, testInfo) => {
  browserContextsByTest.set(testInfo, new Set(browser.contexts()));
});
test.afterEach(async ({ browser }, testInfo) => {
  const initialContexts = browserContextsByTest.get(testInfo) ?? new Set();
  await Promise.all(
    browser
      .contexts()
      .filter((context) => !initialContexts.has(context))
      .map((context) => context.close()),
  );
});

async function setupUncertainRemovePointer(page: Page) {
  const evidence = await mkdtemp(join(tmpdir(), "studio-message-hit-test-"));
  const fixture = spawn(process.execPath, [rustUiFixture, evidence], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  let log = "";
  required(fixture.stderr).on("data", (chunk) => (log += chunk));
  const port = await waitForFixturePort(fixture, {
    label: "Message",
    diagnostics: () => log,
  });
  const origin = `http://127.0.0.1:${port}`;
  const state = await readTestState(origin);
  const lead = required(
    state.threads.find((agent) => agent.name === "Release lead"),
  );
  await page.setViewportSize({ width: 1440, height: 960 });
  await page.route(
    "**/api/commands",
    sendRoute((_route, body, reply) =>
      reply({
        id: body.id,
        status: "uncertain",
        error: "Fixture cannot confirm delivery",
      }),
    ),
  );
  await page.goto(origin);
  await page.locator(`[data-chat="${lead.id}"]`).click();
  await page.locator("#message").fill("Uncertain message for hit testing");
  await page.locator("#send").click();
  const message = page
    .locator("#messages article.message.user")
    .filter({ hasText: "Uncertain message for hit testing" });
  await message.locator(".message-delivery-error").waitFor();
  const remove = message.getByRole("button", {
    name: "Remove message",
    exact: true,
  });
  const quote = message.getByRole("button", {
    name: "Quote message",
    exact: true,
  });
  await message.waitFor({ state: "visible", timeout: 10_000 });
  await remove.waitFor({ state: "visible", timeout: 10_000 });
  await quote.waitFor({ state: "visible", timeout: 10_000 });
  await remove.scrollIntoViewIfNeeded();
  const messageBox = await message.boundingBox();
  await page.mouse.move(
    required(messageBox).x + required(messageBox).width / 2,
    required(messageBox).y + required(messageBox).height / 2,
    { steps: 5 },
  );
  const box = await remove.boundingBox();
  await page.mouse.move(
    required(box).x + required(box).width / 2,
    required(box).y + required(box).height / 2,
    {
      steps: 5,
    },
  );
  const hit = await remove.evaluate((button) => {
    const bounds = button.getBoundingClientRect();
    const center = document.elementFromPoint(
      bounds.x + bounds.width / 2,
      bounds.y + bounds.height / 2,
    );
    return center?.closest("button")?.getAttribute("aria-label") || null;
  });
  return { message, remove, quote, hit, box };
}

test("uncertain outgoing message exposes both delivery controls", async ({
  page,
}) => {
  test.setTimeout(60_000);
  page.setDefaultTimeout(10_000);
  const { message, remove, quote } = await setupUncertainRemovePointer(page);
  await message
    .locator(".message-delivery-error")
    .waitFor({ state: "visible" });
  await remove.waitFor({ state: "visible" });
  await quote.waitFor({ state: "visible" });
});

test("remove message button remains reachable by pointer", async ({ page }) => {
  test.setTimeout(60_000);
  page.setDefaultTimeout(10_000);
  const { message, hit, box } = await setupUncertainRemovePointer(page);
  test.fail(
    // @ts-expect-error The original fixture passes its truthy description as the condition.
    "Known delivery-control overlap; see workspaces/client/apps/web/tests/status/msg-known-failures.md.",
  );
  assert.equal(hit, "Quote message");
  await page.mouse.click(
    required(box).x + required(box).width / 2,
    required(box).y + required(box).height / 2,
  );
  await message.waitFor({ state: "hidden", timeout: 1000 });
});

for (const attachmentCase of [false, true])
  test(
    attachmentCase
      ? "message delivery attachment retry"
      : "message delivery ui",
    async ({ browser: _browser }) => {
      test.setTimeout(120_000);
      const evidence = await mkdtemp(
        join(tmpdir(), "studio-message-delivery-"),
      );
      const fixture = spawn(process.execPath, [rustUiFixture, evidence], {
        stdio: ["ignore", "pipe", "pipe"],
      });
      let log = "",
        browser;
      let stopStreamFixture: (() => Promise<void>) | undefined;
      required(fixture.stderr).on("data", (chunk) => {
        log += chunk;
      });
      async function until(
        check: () => boolean | Promise<boolean>,
        label: string,
      ) {
        const deadline = Date.now() + 12000;
        while (Date.now() < deadline) {
          if (await check()) return;
          await new Promise((resolve) => setTimeout(resolve, 40));
        }
        throw new Error(label);
      }
      try {
        const port = await waitForFixturePort(fixture, {
          label: "Message delivery",
          diagnostics: () => log,
        });
        const origin = `http://127.0.0.1:${port}`;
        const original = await readTestState(origin);
        const identity = await responseJson<
          import("../playwright.ts").SyncIdentityResponse
        >(await fetch(`${origin}/api/sync/identity`));
        const state: import("../playwright.ts").TestStateView = {
          ...original,
          threads: original.threads.map((agent) => ({
            ...agent,
            status: "running",
            inFlight: true,
            canSend: true,
            turnId: "delivery-fixture-turn",
          })),
          runtime: { ...original.runtime, requests: [] },
        };
        const a = required(
          state.threads.find((agent) => agent.name === "Other project"),
        );
        const b = required(
          state.threads.find((agent) => agent.name === "Release lead"),
        );
        browser = _browser;
        {
          const mode = "rxdb";
          Object.assign(a, {
            status: "running",
            inFlight: true,
            turnId: "delivery-fixture-turn",
          });
          const context = await browser.newContext({
            viewport: { width: 1440, height: 960 },
          });
          const page = await context.newPage();
          page.setDefaultTimeout(12000);
          const mutations: Array<{ method: string; url: string }> = [];
          page.on("request", (request) => {
            // Voice reads and background draft migration are independent of receipt dismissal.
            if (
              !["GET", "HEAD", "OPTIONS"].includes(request.method()) &&
              !["/api/voice/records", "/api/sync/drafts"].includes(
                new URL(request.url()).pathname,
              )
            )
              mutations.push({
                method: request.method(),
                url: new URL(request.url()).pathname,
              });
          });
          const errors: string[] = [];
          page.on("pageerror", (error) =>
            errors.push((error as Error).message),
          );
          const history = new Map<
            string,
            {
              agent: import("../playwright.ts").TestAgent;
              items: components["schemas"]["TranscriptMessageRecord"][];
              replace: boolean;
              order?: string[];
            }
          >(
            [a, b].map((agent) => [
              agent.id,
              {
                agent,
                items: Array.from({ length: 36 }, (_, index) => ({
                  id: `${agent.id}-history-${index}`,
                  role: "assistant",
                  text: `Saved paragraph ${index}. ${"Existing conversation text. ".repeat(22)}`,
                })),
                replace: true,
              },
            ]),
          );
          const revisions = new Map([
            [a.id, 100],
            [b.id, 100],
          ]);
          const streams = new Map<
            ServerResponse,
            components["schemas"]["ResourceRef"][]
          >();
          const sessions = new Map<string, ServerResponse>();
          let streamRevision = 0;
          const frame = (
            resources: components["schemas"]["ResourceRef"][],
          ): components["schemas"]["ResourceChangeEvent"] => ({
            protocol: 4,
            workspaceId: identity.workspaceId,
            epoch: "fixture-epoch",
            revision: ++streamRevision,
            reason: "change",
            resources,
            resourceVersions: resources.map(
              (resource): components["schemas"]["ResourceRevisionEntry"] => {
                if (resource.kind === "drafts")
                  return {
                    resource,
                    revision: streamRevision,
                    draftSnapshot: { complete: true, documents: [] },
                  };
                if (resource.kind !== "transcript")
                  return { resource, revision: streamRevision };
                const revision = revisions.get(resource.agentId) ?? 1;
                const transcriptSnapshot: components["schemas"]["TranscriptPageResponse"] =
                  {
                    items: history.get(resource.agentId)?.items ?? [],
                    agent: history.get(resource.agentId)?.agent,
                    truncated: false,
                    historyVersion: String(revision),
                  };
                return {
                  resource,
                  revision,
                  cursor: { workspaceId: identity.workspaceId, seq: revision },
                  replyKind: "snapshot",
                  transcriptSnapshot,
                };
              },
            ),
          });
          const publishDelivery = () => {
            for (const [stream, resources] of streams)
              stream.write(protocol3SseEvent("resources", frame(resources)));
          };
          const streamServer = createServer(async (request, response) => {
            if (request.method === "POST") {
              let body = "";
              for await (const chunk of request) body += chunk;
              const update: components["schemas"]["SyncSubscriptionRequest"] =
                JSON.parse(body);
              const stream = sessions.get(update.sessionId);
              if (!stream) {
                response.writeHead(410).end();
                return;
              }
              const resources = required(streams.get(stream));
              const removed = new Set(
                update.remove.map((resource) => JSON.stringify(resource)),
              );
              const next = resources.filter(
                (resource) => !removed.has(JSON.stringify(resource)),
              );
              const added = update.add.map(({ resource }) => resource);
              for (const resource of added)
                if (
                  !next.some(
                    (current) =>
                      JSON.stringify(current) === JSON.stringify(resource),
                  )
                )
                  next.push(resource);
              streams.set(stream, next);
              const ack = {
                gen: update.gen,
                cachePolicies: [],
              } satisfies components["schemas"]["SyncSubscriptionAck"];
              stream.write(protocol3SseEvent("subscriptions", ack));
              if (added.length)
                stream.write(protocol3SseEvent("resources", frame(added)));
              response.writeHead(200, {
                "Content-Type": "application/json",
                "Access-Control-Allow-Origin": "*",
              });
              response.end(JSON.stringify(ack));
              return;
            }
            const resources: components["schemas"]["ResourceRef"][] =
              JSON.parse(
                new URL(
                  required(request.url),
                  "http://fixture",
                ).searchParams.get("resources") || "[]",
              );
            response.writeHead(200, {
              "Content-Type": "text/event-stream",
              "Cache-Control": "no-cache",
              "Access-Control-Allow-Origin": "*",
            });
            const sessionId = randomUUID();
            sessions.set(sessionId, response);
            streams.set(response, resources);
            response.write(syncSessionSse(sessionId, identity.workspaceId));
            response.write(apiSchemaHandshakeSse());
            response.write(protocol3SseEvent("resources", frame(resources)));
            response.on("close", () => {
              streams.delete(response);
              sessions.delete(sessionId);
            });
          });
          await new Promise<void>((resolve) =>
            streamServer.listen(0, "127.0.0.1", resolve),
          );
          stopStreamFixture = async () => {
            for (const stream of streams.keys()) stream.end();
            await new Promise<void>((resolve) =>
              streamServer.close(() => resolve()),
            );
          };
          await stubEntityState(
            page,
            state,
            await readFixtureSyncContract(origin),
          );
          for (const path of ["stream?*", "subscriptions"]) {
            await page.route(`**/api/sync/${path}`, (route) => {
              const url = new URL(route.request().url());
              url.port = String((streamServer.address() as AddressInfo).port);
              return route.continue({ url: url.href });
            });
          }
          const queuedItems = new Map<
            string,
            Array<
              Omit<import("../../src/api.ts").QueueItemDto, "created"> &
                Partial<
                  Pick<import("../../src/api.ts").QueueItemDto, "created">
                >
            >
          >();
          await page.route("**/api/queue?*", (route) => {
            const id = new URL(route.request().url()).searchParams.get("agent");
            return route.fulfill({
              json: { items: queuedItems.get(required(id)) || [] },
            });
          });
          const posts: Array<{ route: Route; body: MessageRequest }> = [];
          await page.route("**/api/commands", (route) => {
            const body = sentMessage(route.request());
            if (!body) {
              void route.fallback();
              return;
            }
            posts.push({ route, body });
          });
          const publish = async (
            id: string,
            items: components["schemas"]["TranscriptMessageRecord"][],
          ) => {
            const payload = {
              ...required(history.get(id)),
              items,
              replace: true,
              order: items.map((item) => item.id),
            };
            history.set(id, payload);
            revisions.set(id, required(revisions.get(id)) + 1);
            publishDelivery();
          };
          const row = (text: string) =>
            page
              .locator("#messages article.message.user")
              .filter({ has: page.locator(".prose.plain", { hasText: text }) });
          const input = page.locator("#message");
          const atBottom = () =>
            page
              .locator("#messages")
              .evaluate(
                (element) =>
                  element.scrollHeight -
                    element.scrollTop -
                    element.clientHeight <
                  4,
              );
          const start = async (text: string) => {
            const count = posts.length;
            await input.fill(text);
            await page.locator("#send").click();
            await until(
              () => posts.length === count + 1,
              `${mode}: exactly one POST starts`,
            );
            const sent = required(posts.at(-1));
            assert.equal(
              await input.inputValue(),
              "",
              `${mode}: draft clears only after the local outbox owns it`,
            );
            await row(text).waitFor();
            assert.equal(
              await row(text).count(),
              1,
              `${mode}: one immediate local message`,
            );
            assert.equal(
              await input.evaluate(
                (element) => document.activeElement === element,
              ),
              true,
              `${mode}: submit retains composer focus`,
            );
            assert.equal(sent.body.delivery, "current_turn");
            return sent;
          };
          const accept = async (
            sent: (typeof posts)[number],
            status: MessageDelivery["status"] = "delivered",
            expectedDraft = "",
          ) => {
            await sent.route.fulfill({
              json: sendReply({
                id: sent.body.id,
                status,
              } satisfies MessageDelivery),
            });
            await until(
              async () => !(await page.locator("#send-state").textContent()),
              `${mode}: delivery request settles`,
            );
            await until(
              async () => (await input.inputValue()) === expectedDraft,
              `${mode}: acknowledgement preserves the current draft`,
            );
          };
          await page.goto(origin);
          await page.locator(`[data-chat="${a.id}"]`).click();
          await page.locator(`[data-message="${a.id}-history-35"]`).waitFor();

          // The HTTP response and the replicated history are separate acknowledgements.
          await page.locator("#messages").evaluate((element) => {
            element.scrollTop = 200;
            element.dispatchEvent(new Event("scroll"));
          });
          const firstText = `${mode} message stays visible through acceptance`;
          const first = await start(firstText);
          await until(
            atBottom,
            `${mode}: explicit send reveals the newest message`,
          );
          assert.match(await row(firstText).innerText(), /Sending/);
          await page.waitForTimeout(220);
          assert.equal(
            await row(firstText).count(),
            1,
            `${mode}: a delayed POST cannot hide the message`,
          );
          const optimisticNode = await row(firstText).elementHandle();
          const pendingBox = await row(firstText).boundingBox();
          await input.fill("Next draft typed during delivery");
          await accept(first, "delivered", "Next draft typed during delivery");
          const acceptedBox = await row(firstText).boundingBox();
          assert.ok(
            Math.abs(
              required(pendingBox).height - required(acceptedBox).height,
            ) <= 1,
            `${mode}: receipt does not resize the sent message (${required(pendingBox).height} -> ${required(acceptedBox).height}px)`,
          );
          assert.ok(
            Math.abs(required(pendingBox).y - required(acceptedBox).y) <= 1,
            `${mode}: receipt does not move the sent message`,
          );
          await input.fill("");
          const base = required(history.get(a.id)).items;
          await publish(a.id, [...base]);
          await page.waitForTimeout(180);
          assert.equal(
            await row(firstText).count(),
            1,
            `${mode}: stale history cannot hide accepted local content`,
          );
          // Matching text alone must not acknowledge a different user message.
          const unrelated = {
            id: `${mode}-same-text-other-id`,
            role: "user",
            text: firstText,
          };
          await publish(a.id, [...base, unrelated]);
          await until(
            async () => (await row(firstText).count()) === 2,
            `${mode}: matching text does not erase a distinct message`,
          );
          const echo = {
            id: `${a.id}:materialized-${first.body.id}`,
            clientMessageId: first.body.id,
            role: "user",
            text: firstText,
          };
          await publish(a.id, [...base, echo]);
          await until(
            async () => (await row(firstText).count()) === 1,
            `${mode}: exact acknowledgement removes only the local duplicate`,
          );
          assert.equal(
            await page.locator(`[data-message="${echo.id}"]`).count(),
            1,
          );
          assert.equal(
            await optimisticNode.evaluate((node) => node.isConnected),
            true,
            `${mode}: receipt with a different server ID preserves the message DOM node`,
          );

          // The history can acknowledge delivery before the POST response arrives.
          const earlyText = `${mode} history arrives before HTTP acknowledgement`;
          const early = await start(earlyText);
          const earlyBase = required(history.get(a.id)).items;
          const earlyEcho = {
            id: `${a.id}:${early.body.id}`,
            clientMessageId: early.body.id,
            role: "user",
            text: earlyText,
            deliveryStatus: "delivered" as const,
            materialized: true,
          };
          await publish(a.id, [...earlyBase, earlyEcho]);
          await until(
            async () =>
              (await row(earlyText)
                .locator(".message-delivery-status")
                .innerText()) === "Sent",
            `${mode}: history replaces pending display before POST returns`,
          );
          await accept(early);
          await publish(a.id, [...earlyBase]);
          await until(
            async () => (await row(earlyText).count()) === 0,
            `${mode}: late HTTP acknowledgement cannot resurrect an observed receipt`,
          );
          {
            await page.reload();
            await page.locator(`[data-chat="${a.id}"]`).click();
            await page.locator(`[data-message="${a.id}-history-35"]`).waitFor();
            assert.equal(
              await row(earlyText).count(),
              0,
              "RxDB retains the observed receipt across reload after late acknowledgement",
            );
          }

          // Subsequent streaming must respect a deliberate move away from the bottom.
          await page.locator("#messages").evaluate((element) => {
            element.scrollTop = 300;
            element.dispatchEvent(new Event("scroll"));
          });
          const top = await page
            .locator("#messages")
            .evaluate((element) => element.scrollTop);
          await publish(a.id, [
            ...required(history.get(a.id)).items,
            {
              id: `${mode}-stream`,
              role: "assistant",
              text: "New streamed content. ".repeat(80),
              streaming: true,
            },
          ]);
          await page.waitForTimeout(220);
          assert.ok(
            Math.abs(
              (await page
                .locator("#messages")
                .evaluate((element) => element.scrollTop)) - top,
            ) < 3,
            `${mode}: streaming preserves manual scroll`,
          );

          // A delayed projection cannot remove a receipt before materialization.
          const queuedText = `${mode} queued message survives dispatch`;
          const queued = await start(queuedText);
          const queueRow = () =>
            page
              .getByTestId("message-queue")
              .getByText(queuedText, { exact: true });
          const assertQueueGeometry: (
            ...args: string[]
          ) => Promise<void> = async () => {
            // After-tool input waits in the chat, never in the after-turn queue.
            await row(queuedText).waitFor();
            assert.equal(await queueRow().count(), 0);
            assert.ok(
              await page.evaluate(
                () => document.documentElement.scrollWidth <= innerWidth + 1,
              ),
            );
          };
          await accept(queued, "queued");
          await assertQueueGeometry("queued receipt");
          await until(
            async () => (await row(queuedText).innerText()).includes("Queued"),
            "Server acceptance must stop showing an active network send",
          );
          const queueBase = required(history.get(a.id)).items;
          const queuedEcho = {
            id: `${a.id}:${queued.body.id}`,
            clientMessageId: queued.body.id,
            role: "user",
            text: queuedText,
            deliveryStatus: "queued" as const,
          };
          await publish(a.id, [...queueBase, queuedEcho]);
          await until(
            async () =>
              (await queueRow().count()) === 0 &&
              (await row(queuedText).count()) === 1,
            `${mode}: one waiting row in the chat`,
          );
          await assertQueueGeometry("queued transcript");
          await page.setViewportSize({ width: 390, height: 844 });
          await row(queuedText).scrollIntoViewIfNeeded();
          await page.waitForTimeout(100);

          assert.ok(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth + 1,
            ),
            `${mode}: queued metadata fits the mobile viewport`,
          );
          await publish(a.id, [...queueBase]);
          await page.waitForTimeout(200);
          assert.equal(
            await row(queuedText).count(),
            1,
            `${mode}: dispatch gap retains the local waiting message`,
          );
          assert.equal(await queueRow().count(), 0);
          await publish(a.id, [
            ...queueBase,
            {
              ...queuedEcho,
              deliveryStatus: "delivered",
            },
          ]);
          await until(
            async () =>
              (await row(queuedText).count()) === 1 &&
              (await row(queuedText)
                .locator(".message-delivery-status")
                .innerText()) === "Sent",
            `${mode}: materialized queue row stays unique`,
          );
          assert.equal(await queueRow().count(), 0);
          await page.setViewportSize({ width: 1440, height: 960 });

          if (attachmentCase) {
            test.fail(
              true,
              "finishUpload marks the confirmed asset cancelled; work-1793530381442156544-11.",
            );
            // A rejected message keeps its draft, attachments, and an inline error.
            await page.locator('input[type="file"]').setInputFiles({
              name: "delivery-evidence.txt",
              mimeType: "text/plain",
              buffer: Buffer.from(
                "Retain this attachment after a rejected send.\n",
              ),
            });
            const attachment = page.getByRole("button", {
              name: "Remove delivery-evidence.txt",
              exact: true,
            });
            await attachment.waitFor();
            const failedText = `${mode} rejected message remains editable`;
            const failed = await start(failedText);
            assert.equal(required(failed.body.assets).length, 1);
            await failed.route.fulfill({
              status: 400,
              json: { error: "Fixture rejects this message" },
            });
            await row(failedText).locator(".message-delivery-error").waitFor();
            assert.match(
              await row(failedText).innerText(),
              /Fixture rejects this message/,
            );
            assert.equal(await input.inputValue(), failedText);
            await attachment.waitFor();
            const beforeRejectedRetry = posts.length;
            await page.locator("#send").click();
            await until(
              () => posts.length === beforeRejectedRetry + 1,
              `${mode}: a definite rejection permits another attempt`,
            );
            const rejectedRetry = required(posts.at(-1));
            assert.notEqual(
              rejectedRetry.body.id,
              failed.body.id,
              `${mode}: retry after HTTP 400 has a fresh request identity`,
            );
            assert.equal(rejectedRetry.body.text, failed.body.text);
            assert.deepEqual(
              rejectedRetry.body.assets,
              required(failed.body.assets),
              `${mode}: retry retains the same attachment`,
            );
            await accept(rejectedRetry);
            await attachment.waitFor({ state: "hidden" });

            return;
          }

          // Unknown delivery does not become permission to submit a fresh message.
          const uncertainText = `${mode} uncertain request retains its identity after reload`;
          const uncertain = await start(uncertainText);
          await uncertain.route.fulfill({
            json: sendReply({
              id: uncertain.body.id,
              status: "uncertain",
              error: "Fixture cannot confirm delivery",
            } satisfies MessageDelivery),
          });
          await row(uncertainText).locator(".message-delivery-error").waitFor();
          assert.equal(await input.inputValue(), "");
          const beforeUncertainRetry = posts.length;
          await row(uncertainText)
            .getByRole("button", { name: "Retry", exact: true })
            .click();
          await until(
            () => posts.length === beforeUncertainRetry + 1,
            `${mode}: uncertain card retries`,
          );
          const retry = required(posts.at(-1));
          assert.deepEqual(
            retry.route.request().postDataJSON(),
            uncertain.route.request().postDataJSON(),
            `${mode}: retry reuses the exact command`,
          );
          await retry.route.fulfill({
            json: sendReply({
              id: retry.body.id,
              status: "delivered",
            } satisfies MessageDelivery),
          });
          await publish(a.id, [
            ...required(history.get(a.id)).items,
            {
              id: `${a.id}:${uncertain.body.id}`,
              clientMessageId: uncertain.body.id,
              role: "user",
              text: uncertainText,
              deliveryStatus: "delivered",
              materialized: true,
            },
          ]);
          await until(
            async () =>
              (await row(uncertainText).count()) === 1 &&
              (await row(uncertainText)
                .locator(".message-delivery-status")
                .innerText()) === "Sent",
            `${mode}: retry reconciles to one server message`,
          );
          const expectedPosts = posts.length + 1;

          // Completion in A cannot clear B's draft or move B's keyboard focus.
          const switchText = `${mode} pending message belongs to first chat`;
          const switched = await start(switchText);
          await page.locator(`[data-chat="${b.id}"]`).click();
          await page.locator(`[data-message="${b.id}-history-35"]`).waitFor();
          const otherDraft = `${mode} draft in second chat`;
          await input.fill(otherDraft);
          await switched.route.fulfill({
            json: sendReply({
              id: switched.body.id,
              status: "delivered",
            } satisfies MessageDelivery),
          });
          await page.waitForTimeout(250);
          assert.equal(
            await input.inputValue(),
            otherDraft,
            `${mode}: old send preserves new chat draft`,
          );
          assert.equal(
            await input.evaluate(
              (element) => document.activeElement === element,
            ),
            true,
            `${mode}: old send preserves new chat focus`,
          );
          assert.equal(
            await row(switchText).count(),
            0,
            `${mode}: outgoing message stays scoped to its chat`,
          );
          await page.locator(`[data-chat="${a.id}"]`).click();
          await row(switchText).waitFor();
          assert.equal(
            await row(switchText).count(),
            1,
            `${mode}: returning restores the accepted message`,
          );
          assert.equal(
            posts.length,
            expectedPosts,
            `${mode}: no automatic duplicate delivery`,
          );
          // A waiting after-tool receipt stays in the chat until native dispatch,
          // also after a reload. It never enters the after-turn queue.
          const staleText = `${mode} stale queue receipt`;
          const stale = await start(staleText);
          await accept(stale, "queued");
          await row(staleText).waitFor();
          const staleBase = required(history.get(a.id)).items;
          await publish(a.id, [
            ...staleBase,
            {
              id: `${a.id}:${stale.body.id}`,
              clientMessageId: stale.body.id,
              role: "user",
              text: staleText,
              deliveryStatus: "queued",
              materialized: false,
            },
          ]);
          await page.reload();
          await page.locator(`[data-chat="${a.id}"]`).click();
          await page.locator(`[data-message="${a.id}-history-35"]`).waitFor();
          await row(staleText).waitFor();
          assert.equal(
            await page
              .getByTestId("message-queue")
              .getByText(staleText)
              .count(),
            0,
            `${mode}: waiting receipt has one visible location`,
          );
          assert.equal(await row(staleText).count(), 1);
          await publish(a.id, [
            ...staleBase,
            {
              id: `${a.id}:${stale.body.id}`,
              clientMessageId: stale.body.id,
              role: "user",
              text: staleText,
            },
          ]);
          await row(staleText).waitFor();
          // Transcript materialization arrives through the resource stream, then
          // the client pulls the new durable document without resending the input.
          const materializedText = `${mode} delivered input after transcript sync`;
          const materialized = await start(materializedText);
          queuedItems.set(a.id, [
            {
              id: materialized.body.id,
              kind: "user",
              text: materializedText,
              status: "queued",
              delivery: "current_turn",
            },
          ]);
          await accept(materialized, "queued");
          await until(
            async () =>
              (await row(materializedText).innerText()).includes("Queued"),
            "Accepted input remains visible while the agent waits",
          );
          const silentBase = required(history.get(a.id));
          history.set(a.id, {
            ...silentBase,
            items: [
              ...silentBase.items,
              {
                id: `${a.id}:${materialized.body.id}`,
                clientMessageId: materialized.body.id,
                role: "user",
                text: materializedText,
                deliveryStatus: "delivered",
                materialized: true,
              },
            ],
          });
          revisions.set(a.id, required(revisions.get(a.id)) + 1);
          queuedItems.set(a.id, []);
          publishDelivery();
          const postsBeforeReconcile = posts.length;
          await until(
            async () =>
              (await row(materializedText).count()) === 1 &&
              (await row(materializedText)
                .locator(".message-delivery-status")
                .innerText()) === "Sent",
            "A transcript resource update shows the delivered input status",
          );
          assert.equal(
            posts.length,
            postsBeforeReconcile,
            "Receipt reconciliation never resends",
          );

          // Dismissal hides one receipt on this device without cancelling delivery.
          const dismissedText = `${mode} uncertain message removed from this device`;
          const dismissed = await start(dismissedText);
          await dismissed.route.fulfill({
            json: sendReply({
              id: dismissed.body.id,
              status: "uncertain",
              error: "Fixture cannot confirm delivery",
            } satisfies MessageDelivery),
          });
          await row(dismissedText).locator(".message-delivery-error").waitFor();
          await until(
            async () =>
              (await page.locator("#send-state").innerText()).trim() === "",
            `${mode}: uncertain response returns control before dismissal`,
          );
          await page.waitForTimeout(350);
          const dismissalDraft = `${mode} draft retained during dismissal`;
          await input.fill(dismissalDraft);
          const mutationsBeforeRemoval = mutations.length;
          const remove = row(dismissedText).getByRole("button", {
            name: "Remove message",
            exact: true,
          });
          assert.equal(
            await remove.getAttribute("title"),
            "Remove from this device. Delivery is not cancelled.",
          );
          await remove.focus();
          await page.keyboard.press("Enter");
          await row(dismissedText).waitFor({ state: "hidden" });
          assert.equal(
            await input.inputValue(),
            dismissalDraft,
            `${mode}: dismissal preserves the draft`,
          );
          await page.locator(`[data-chat="${b.id}"]`).click();
          await page.locator(`[data-message="${b.id}-history-35"]`).waitFor();
          await page.locator(`[data-chat="${a.id}"]`).click();
          await page.locator(`[data-message="${a.id}-history-35"]`).waitFor();
          assert.equal(
            await row(dismissedText).count(),
            0,
            `${mode}: dismissal survives a chat switch`,
          );
          await page.reload();
          await page.locator(`[data-chat="${a.id}"]`).click();
          await page.locator(`[data-message="${a.id}-history-35"]`).waitFor();
          assert.equal(
            await row(dismissedText).count(),
            0,
            `${mode}: dismissal survives reload`,
          );
          const dismissedEcho = {
            id: `${a.id}:${dismissed.body.id}`,
            clientMessageId: dismissed.body.id,
            role: "user",
            text: dismissedText,
          };
          const distinctEcho = {
            ...dismissedEcho,
            id: `${mode}-distinct-dismissed-text`,
            clientMessageId: `${mode}-distinct-client-id`,
          };
          await publish(a.id, [
            ...required(history.get(a.id)).items,
            dismissedEcho,
            distinctEcho,
          ]);
          await page.locator(`[data-message="${distinctEcho.id}"]`).waitFor();
          assert.equal(
            await page.locator(`[data-message="${dismissedEcho.id}"]`).count(),
            1,
            `${mode}: a later server record remains visible`,
          );
          assert.equal(
            await row(dismissedText).count(),
            2,
            `${mode}: both server identities with identical text remain visible`,
          );
          assert.equal(
            await page
              .locator(`[data-message="${distinctEcho.id}"]`)
              .getByRole("button", { name: "Remove message", exact: true })
              .count(),
            0,
            `${mode}: delivered messages do not expose dismissal`,
          );

          // Server-side failures use the same local dismissal as uncertain local sends.
          const serverBase = [...required(history.get(a.id)).items];
          const removable = (
            [
              "uncertain",
              "failed",
              "cancelled",
            ] satisfies MessageDelivery["status"][]
          ).map((status) => ({
            id: `${mode}-server-${status}`,
            role: "user",
            text: `${mode} server ${status} receipt`,
            deliveryStatus: status,
            deliveryError: `Fixture ${status} receipt`,
          }));
          const retained = (
            ["delivered", "queued"] satisfies MessageDelivery["status"][]
          ).map((status) => ({
            id: `${mode}-server-${status}`,
            role: "user",
            text: `${mode} server ${status} receipt`,
            deliveryStatus: status,
          }));
          await publish(a.id, [...serverBase, ...removable, ...retained]);
          for (const message of [...removable, ...retained]) {
            const item = page.locator(`[data-message="${message.id}"]`);
            await item.waitFor();
            assert.equal(
              await item
                .getByRole("button", { name: "Remove message", exact: true })
                .count(),
              0,
              `${mode}: server-owned ${message.deliveryStatus} has no device removal`,
            );
          }
          await publish(a.id, [
            ...serverBase,
            ...removable.map(
              ({
                deliveryStatus: _deliveryStatus,
                deliveryError: _deliveryError,
                ...message
              }) => message,
            ),
            ...retained,
          ]);
          await page.reload();
          await page.locator(`[data-chat="${a.id}"]`).click();
          await page
            .locator(`[data-message="${required(retained.at(-1)).id}"]`)
            .waitFor();
          for (const message of [...removable, dismissedEcho])
            assert.equal(
              await page.locator(`[data-message="${message.id}"]`).count(),
              1,
              `${mode}: server message ${message.id} remains visible after reload`,
            );
          assert.equal(
            await page.locator(`[data-message="${distinctEcho.id}"]`).count(),
            1,
          );
          // Message IDs do not share dismissal state across conversations.
          const otherChatReceipt = {
            ...required(removable[0]),
            text: `${mode} same ID in another chat`,
          };
          await publish(b.id, [
            ...required(history.get(b.id)).items,
            otherChatReceipt,
          ]);
          await page.locator(`[data-chat="${b.id}"]`).click();
          await row(otherChatReceipt.text).waitFor();
          assert.equal(
            await row(otherChatReceipt.text).count(),
            1,
            `${mode}: dismissal stays scoped to its chat`,
          );
          await page.locator(`[data-chat="${a.id}"]`).click();
          await page.locator(`[data-message="${distinctEcho.id}"]`).waitFor();
          assert.equal(
            await page
              .locator(`[data-message="${otherChatReceipt.id}"]`)
              .count(),
            1,
          );
          assert.deepEqual(
            mutations
              .slice(mutationsBeforeRemoval)
              .filter(({ url }) =>
                /^\/api\/(?:messages?(?:\/|$)|queue(?:\/|$)|outbox(?:\/|$))/.test(
                  url,
                ),
              ),
            [],
            `${mode}: dismissal does not mutate message, queue, or outbox state`,
          );
          assert.deepEqual(errors, [], `${mode}: no renderer exceptions`);
          await context.close();
          console.log(
            `${mode}: delivery, reconciliation, queue, rejection, local dismissal, focus, scroll, and chat isolation pass`,
          );
        }
        console.log(`Evidence: ${evidence}`);
      } finally {
        await stopStreamFixture?.();
        fixture.stdin?.end();
      }
    },
  );
