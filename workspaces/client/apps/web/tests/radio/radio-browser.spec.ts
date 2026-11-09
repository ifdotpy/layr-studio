import type {} from "./radio-browser.fixture.tsx";
import { createFixtureViteServer as createServer } from "../playwright.ts";
import type { Room } from "../../src/types.ts";
import type { PostBody } from "../../src/api.ts";
import { fixturePostBody } from "../misc-fixture.ts";
import { fixtureOrigin } from "../misc-fixture.ts";

import {
  test,
  expect,
  API_SCHEMA_HASH_HEADER,
  readApiSchemaHash,
  handleEntitySyncFixtureRequest,
  syncIdentityFixture,
} from "../playwright.ts";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
test("Radio browser", async ({
  browser: testBrowser,
  page: runnerPage,
  context: runnerContext,
}) => {
  test.setTimeout(180_000);
  const root = resolve(import.meta.dirname, "../../");

  const cacheDir = await mkdtemp(join(tmpdir(), "studio-radio-"));
  let streamConnections = 0;
  const streams = new Set<import("node:http").ServerResponse>();
  const roomMessages = [
    {
      id: "1",
      seq: 1,
      sender: "user",
      text: "Compare these approaches",
      created: 1750000000,
    },
    {
      id: "2",
      seq: 2,
      sender: "a",
      senderName: "Astra",
      text: "First reply. **Keep this visible.**",
      created: 1750000010,
    },
    {
      id: "3",
      seq: 3,
      sender: "b",
      senderName: "Claude",
      text: "Second reply with a different view.",
      created: 1750000020,
    },
  ];
  const roomRef = { kind: "room", roomId: "radio:t" } as const;
  const snapshot = {
    stateDir: "radio-test",
    chats: [],
    edges: [],
    runtime: {},
  };
  let notifyRoom = () => {};
  const server = await createServer({
    configFile: false,
    root,
    cacheDir,
    server: { host: "127.0.0.1", port: 0, hmr: false },
    plugins: [
      {
        name: "radio-fixture",
        configureServer(s) {
          s.middlewares.use((req, res, next) => {
            if (
              new URL(req.url!, "http://fixture").pathname ===
              "/api/sync/stream"
            ) {
              streamConnections++;
              streams.add(res);
              res.on("close", () => streams.delete(res));
            }
            if (
              !handleEntitySyncFixtureRequest(req, res, {
                snapshot,
                workspaceId: "1234567890abcdef1234567890abcdef",
                roomSnapshot: () => ({
                  room: { id: "radio:t", members: ["a", "b"] },
                  messages: roomMessages.map((message) => ({
                    ...message,
                    senderName: message.senderName ?? "You",
                    room: "radio:t",
                    deliveries: {},
                  })),
                  nextBefore: null,
                  nextAfter: null,
                }),
                onStreamReady: (notify) => {
                  notifyRoom = () => notify([roomRef]);
                },
              })
            )
              next();
          });
          s.middlewares.use("/check", (_req, res) => {
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<html><body><div id="root"></div><script type="module" src="/tests/radio/radio-browser.fixture.tsx"></script></body></html>',
            );
          });
        },
      },
    ],
  });
  try {
    await server.listen();
    const page = runnerPage;
    page.setDefaultTimeout(10000);
    await page.setViewportSize({ width: 1100, height: 850 });
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    let mode = "lost";
    let releaseResponse: (() => void) | undefined;
    const requests: PostBody<"/api/peer-teams">[] = [];
    await page.route("**/api/sync/identity", (r) =>
      r.fulfill({
        headers: { [API_SCHEMA_HASH_HEADER]: readApiSchemaHash() },
        json: syncIdentityFixture("1234567890abcdef1234567890abcdef"),
      }),
    );
    await page.route("**/api/agent-chat?*", (r) =>
      r.fulfill({
        json: {
          messages: roomMessages,
          nextBefore: null,
        },
      }),
    );
    await page.route("**/snapshot", async (r) =>
      r.fulfill({
        json: await page.evaluate(() => window.currentRoom),
      }),
    );
    await page.route("**/api/peer-teams", async (r) => {
      requests.push(
        fixturePostBody<"/api/peer-teams">(r.request().postDataJSON()),
      );
      if (mode === "lost") {
        mode = "ok";
        return r.abort("failed");
      }
      if (mode === "reject")
        return r.fulfill({ status: 409, json: { error: "Revision changed" } });
      if (mode === "held")
        await new Promise<void>((resolve) => {
          releaseResponse = resolve;
        });
      return r.fulfill({ json: { room: { id: "radio:t" } } });
    });
    await page.goto(`${fixtureOrigin(server)}/check`, {
      waitUntil: "domcontentloaded",
      timeout: 30000,
    });
    await page.getByText("Second reply with a different view.").waitFor();
    assert.deepEqual(
      await page
        .locator(".radio-message")
        .evaluateAll((es) => es.map((e) => e.getAttribute("data-sender"))),
      ["user", "a", "b"],
    );
    assert.equal(
      await page.locator(".radio-message time[datetime]").count(),
      3,
    );
    await page
      .getByRole("button", { name: "Options for team Research" })
      .click();
    await page.getByRole("menuitem", { name: "Open shared chat" }).click();
    await page.waitForFunction(() => window.radioOpened === "radio:t");
    assert.equal(
      requests.length,
      0,
      "The published room opens without a refresh or mutation",
    );
    requests.length = 0;
    mode = "lost";
    const input = page.getByRole("textbox", { name: "Message both agents" });
    await input.fill("Hello both");
    await page.getByRole("button", { name: "Send", exact: true }).click();
    await page.getByRole("button", { name: "Retry", exact: true }).waitFor();
    assert.equal(await input.inputValue(), "Hello both");
    assert.equal(await input.isDisabled(), true);
    const reopenedStream = page.waitForResponse(
      (response) => new URL(response.url()).pathname === "/api/sync/stream",
    );
    await page.reload({ waitUntil: "domcontentloaded", timeout: 30000 });
    await reopenedStream;
    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await page.waitForFunction(() => !document.querySelector(".radio-retry"));
    assert.equal(requests.length, 2);
    assert.deepEqual(requests[0], requests[1]);
    assert.equal(
      (
        requests[0] as Extract<
          PostBody<"/api/peer-teams">,
          { radio_action: "send" }
        >
      ).rounds,
      1,
    );
    assert.equal(
      (
        requests[0] as Extract<
          PostBody<"/api/peer-teams">,
          { radio_action: "send" }
        >
      ).target,
      "both",
    );
    const connectionsBeforeCommand = streamConnections;
    mode = "held";
    await page.evaluate(() => {
      window.holdRefresh = true;
      window.refreshCalls = 0;
    });
    await input.fill("Confirmed send");
    await page.getByRole("button", { name: "Send", exact: true }).click();
    await expect.poll(() => typeof releaseResponse).toBe("function");
    assert.equal(await input.inputValue(), "Confirmed send");
    assert.equal(await input.isDisabled(), true);
    assert.equal(await page.evaluate(() => window.refreshCalls), 0);
    const acceptedPosts = requests.length;
    releaseResponse!();
    await expect(input).toBeEnabled({ timeout: 2000 });
    assert.equal(await input.inputValue(), "");
    roomMessages.push({
      id: "4",
      seq: 4,
      sender: "b",
      senderName: "Claude",
      text: "SSE reply after send",
      created: 1750000030,
    });
    notifyRoom();
    await expect(page.getByText("SSE reply after send")).toBeVisible();
    assert.equal(await page.evaluate(() => window.refreshCalls), 0);
    assert.deepEqual(await page.evaluate(() => window.notifications || []), []);
    assert.equal(
      streamConnections,
      connectionsBeforeCommand,
      "A successful command keeps its SSE connection",
    );
    assert.equal(requests.length, acceptedPosts);
    assert.equal(
      await page.evaluate(() =>
        JSON.parse(
          localStorage.getItem("studio-radio-command:radio-test:radio:t")!,
        ),
      ),
      null,
    );
    mode = "ok";
    await input.fill("Discuss tradeoffs");
    await page.evaluate(() => (window.failRefresh = true));
    await page
      .getByRole("button", { name: "Discuss (4 replies)", exact: true })
      .click();
    await expect(input).toBeEnabled();
    assert.equal(await page.evaluate(() => window.refreshCalls), 0);
    assert.deepEqual(await page.evaluate(() => window.notifications || []), []);
    assert.equal(await input.inputValue(), "");
    const posted = requests.length;
    await page.evaluate(() => (window.failRefresh = false));
    assert.equal(await page.locator(".radio-retry").count(), 0);
    assert.equal(requests.length, posted);
    assert.equal(
      (
        requests.at(-1)! as Extract<
          PostBody<"/api/peer-teams">,
          { radio_action: "send" }
        >
      ).rounds,
      2,
    );
    mode = "reject";
    await input.fill("Stale proposal");
    await page.getByRole("button", { name: "Send", exact: true }).click();
    await page.getByRole("button", { name: "Refresh", exact: true }).waitFor();
    const rejected = requests.length;
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await page.waitForFunction(() => !document.querySelector(".radio-retry"));
    assert.equal(requests.length, rejected);
    assert.equal(await input.inputValue(), "Stale proposal");
    mode = "ok";
    await page.evaluate(() =>
      window.setRoom({
        ...window.currentRoom,
        radio: {
          ...window.currentRoom.radio!,
          status: "speaking",
          controls: {
            busy: true,
            inputBlocked: false,
            canPass: true,
            stopping: false,
            speaking: true,
            busyNotice: "Current reply finishes first.",
          },
          speaker: "a",
        },
      }),
    );
    await page.getByRole("button", { name: "Reply next" }).click();
    await page.waitForFunction(() => !document.querySelector(".radio-retry"));
    assert.equal(
      (
        requests.at(-1)! as Extract<
          PostBody<"/api/peer-teams">,
          { radio_action: "pass" }
        >
      ).radio_action,
      "pass",
    );
    assert.equal(
      (
        requests.at(-1)! as Extract<
          PostBody<"/api/peer-teams">,
          { radio_action: "pass" }
        >
      ).target,
      "b",
    );
    await page.getByRole("button", { name: "Stop", exact: true }).click();
    await page.waitForFunction(() => !document.querySelector(".radio-retry"));
    assert.equal(
      (
        requests.at(-1)! as Extract<
          PostBody<"/api/peer-teams">,
          { radio_action: "stop" }
        >
      ).radio_action,
      "stop",
    );
    assert.equal(
      await page.getByRole("button", { name: "Send", exact: true }).isEnabled(),
      true,
    );
    await page.getByRole("button", { name: "Send", exact: true }).click();
    await page.waitForFunction(() => !document.querySelector(".radio-retry"));
    assert.equal(
      (
        requests.at(-1)! as Extract<
          PostBody<"/api/peer-teams">,
          { radio_action: "send" }
        >
      ).radio_action,
      "send",
    );
    await input.fill("New instruction");
    await page.evaluate(() =>
      window.setRoom({
        ...window.currentRoom,
        radio: {
          ...window.currentRoom.radio!,
          status: "blocked",
          controls: {
            busy: false,
            inputBlocked: false,
            canPass: false,
            stopping: false,
            speaking: false,
            busyNotice: null,
          },
          error: "Reply interrupted. Send a new message.",
          active: null,
          speaker: null,
        },
      }),
    );
    assert.equal(
      await page.getByRole("button", { name: "Send", exact: true }).isEnabled(),
      true,
    );
    await page.evaluate(
      (active) =>
        window.setRoom({
          ...window.currentRoom,
          radio: {
            ...window.currentRoom.radio!,
            status: "blocked",
            controls: {
              busy: true,
              inputBlocked: true,
              canPass: false,
              stopping: false,
              speaking: false,
              busyNotice: "Resolve the current reply before sending.",
            },
            active,
            speaker: "a",
          },
        }),
      radioActive(),
    );
    assert.equal(
      await page
        .getByRole("button", { name: "Send", exact: true })
        .isDisabled(),
      true,
    );
    assert.equal(
      await page.getByRole("button", { name: "Stop", exact: true }).isEnabled(),
      true,
    );
    await page.evaluate(
      (active) =>
        window.setRoom({
          ...window.currentRoom,
          radio: {
            ...window.currentRoom.radio!,
            status: "speaking",
            controls: {
              busy: true,
              inputBlocked: false,
              canPass: true,
              stopping: false,
              speaking: true,
              busyNotice: "Current reply finishes first.",
            },
            error: null,
            active,
            speaker: "a",
          },
        }),
      radioActive(),
    );
    await page
      .getByRole("button", { name: "Options for team Research" })
      .click();
    await page.getByRole("menuitem", { name: "Open shared chat" }).click();
    await page.waitForFunction(() => window.radioOpened === "radio:t");
    await page.getByRole("menu").waitFor({ state: "hidden" });
    for (const width of [1100, 375]) {
      await page.setViewportSize({ width, height: 850 });
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
        true,
      );
      assert.equal(
        await page
          .getByRole("button", { name: "Send", exact: true })
          .evaluate(
            (e) =>
              e.getBoundingClientRect().bottom <=
              document.getElementById("root")!.getBoundingClientRect().bottom,
          ),
        true,
      );
      await page.screenshot({
        path: join(tmpdir(), `studio-radio-${width}.png`),
        fullPage: true,
      });
    }
    assert.deepEqual(errors, []);
    console.log(
      `PASS radio UI: ordered transcript, timestamps, durable exact retry, SSE publication without refresh, rejected revision, pass, stop, open and responsive layout. Screenshots ${tmpdir()}/studio-radio-{1100,375}.png`,
    );
  } finally {
    await runnerPage.close();
    for (const stream of streams) stream.end();
    await Promise.all(
      testBrowser
        .contexts()
        .filter((ownedContext) => ownedContext !== runnerContext)
        .map((ownedContext) => ownedContext.close()),
    );
    await server.close();
    await rm(cacheDir, { recursive: true, force: true });
  }
});

// The two base speaking/blocked fixtures omit the remaining active-turn metadata.
function radioActive() {
  return { eventId: "e", agentId: "a" } as NonNullable<
    NonNullable<Room["radio"]>["active"]
  >;
}
