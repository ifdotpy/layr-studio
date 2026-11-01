import type { components } from "../../src/generated/api.ts";
import { backgroundDisplay } from "../fixtures/background.ts";
import { approval } from "../../src/approvalFixtures.ts";
import { required } from "../teams/helpers.ts";
import { execFileSync } from "node:child_process";
import { fixtureSnapshot } from "../fixtures/snapshot.ts";
import { createServer } from "vite";
import type {} from "./background-current-browser.fixture.tsx";
// Current activity lifecycle through the real component. HTTP calls use fixtures.
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  test,
  expect,
  API_SCHEMA_HASH_HEADER,
  readApiSchemaHash,
} from "../playwright.ts";

test("Background Current Browser", async ({
  browser: _testBrowser,
  context: _testContext,
  page: testPage,
}, testInfo) => {
  const assert = {
    equal: (actual: unknown, expected: unknown, message?: string) =>
      expect(actual, message).toBe(expected),
    notEqual: (actual: unknown, expected: unknown, message?: string) =>
      expect(actual, message).not.toBe(expected),
    deepEqual: (actual: unknown, expected: unknown, message?: string) =>
      expect(actual, message).toEqual(expected),
    ok: (actual: unknown, message?: string) =>
      expect(actual, message).toBeTruthy(),
    match: (actual: string, expected: RegExp, message?: string) =>
      expect(actual, message).toMatch(expected),
    doesNotMatch: (actual: string, expected: RegExp, message?: string) =>
      expect(actual, message).not.toMatch(expected),
    fail: (message: string) => {
      throw new Error(message);
    },
  };

  const root = fileURLToPath(new URL("../../../../../../", import.meta.url));
  const temporary = await mkdtemp(join(tmpdir(), "studio-current-background-"));

  const server = await createServer({
    configFile: false,
    root: join(root, "workspaces/client/apps/web"),
    cacheDir: join(temporary, "vite"),
    plugins: [
      {
        name: "current-background-fixture",
        load(id) {
          if (
            process.env.BASELINE_SYNC_WAIT === "1" &&
            id ===
              join(
                root,
                "workspaces/client/apps/web/src/components/shell/BackgroundTasks.tsx",
              )
          )
            return execFileSync(
              "git",
              [
                "show",
                "HEAD:workspaces/client/apps/web/src/components/shell/BackgroundTasks.tsx",
              ],
              { cwd: root, encoding: "utf8" },
            );
          return undefined;
        },
      },
    ],
    server: { host: "127.0.0.1", port: 0, hmr: false },
  });
  await server.listen();
  try {
    const page = testPage;
    await page.setViewportSize({ width: 1440, height: 900 });
    const errors: string[] = [],
      calls: { method: string; path: string; id: string | null }[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.route("**/fixture", (route) =>
      route.fulfill({
        contentType: "text/html",
        body: '<!doctype html><div id="root"></div><script type="module">import "/tests/background/background-current-browser.fixture.tsx";</script>',
      }),
    );
    let holdAnswer = false,
      releaseAnswer: (() => void) | undefined;
    let currentListing: components["schemas"]["ProcessListing"];
    await page.route("**/api/**", async (route) => {
      const request = route.request();
      const url = new URL(request.url());
      calls.push({
        method: request.method(),
        path: url.pathname,
        id: url.searchParams.get("id"),
      });
      if (url.pathname === "/api/sync/identity")
        return route.fulfill({
          headers: { [API_SCHEMA_HASH_HEADER]: readApiSchemaHash() },
          json: {
            workspaceId: "1234567890abcdef1234567890abcdef",
            syncProtocol: 2,
          },
        });
      if (url.pathname === "/api/workspace/tasks")
        return route.fulfill({
          status: 503,
          json: { error: "Fixture uses embedded task snapshots" },
        });
      if (url.pathname === "/api/processes")
        return route.fulfill({ json: currentListing });
      if (
        request.method() === "POST" &&
        url.pathname === "/api/answer" &&
        holdAnswer
      )
        await new Promise<void>((resolve) => {
          releaseAnswer = resolve;
        });
      return route.fulfill({
        json: { id: url.searchParams.get("id"), tail: "Live fixture output" },
      });
    });
    await page.goto(server.resolvedUrls!.local[0]! + "fixture");
    await page.locator('#root[data-ready="true"]').waitFor();
    const agents: import("../../src/generated/api.ts").AgentEntityDto[] = [
      { id: "lead", rootId: "lead", name: "Lead" },
      { id: "worker", rootId: "lead", name: "Worker" },
      { id: "other", rootId: "other", name: "Other" },
    ];
    const task = (
      id: string,
      status: NonNullable<
        import("../../src/types.ts").BackgroundTask["status"]
      >,
      agent = "lead",
    ): import("../../src/types.ts").BackgroundTask => ({
      id,
      status,
      agent,
      display: backgroundDisplay("command", "command " + id),
      kind: "command",
      command: "command " + id,
      tail: "Summary output " + id,
      created: Date.now() / 1000 - 30,
      processId: "123",
    });
    const data: import("../../src/types.ts").Snapshot = {
      ...fixtureSnapshot(agents),
      token: "fixture",
      threads: agents,
      runtime: {
        ...fixtureSnapshot(agents).runtime,
        requests: [
          approval({
            id: "approve-id",
            request: {
              kind: "monitor_start",
              monitor: "approval",
              command: "tail -f log",
            },
          }),
        ],
        monitors: [
          {
            ...task("approval", "approval"),
            kind: "monitor",
          } as import("../../src/types.ts").Monitor,
        ],
        tasks: [
          task("active", "running"),
          {
            ...task("tool", "running"),
            kind: "tool",
            name: "webSearch",
            command: undefined,
          },
          task("child", "running", "worker"),
          task("other-active", "running", "other"),
          ...(["completed", "failed", "interrupted", "lost"] as const).map(
            (status) => task(status, status),
          ),
          // @ts-expect-error cancelled is a deliberately legacy task status.
          task("cancelled", "cancelled"),
          task("starting", "starting"),
          task("pending", "pending"),
          // @ts-expect-error stopping is a deliberately legacy task status.
          task("stopping", "stopping"),
        ],
      },
    };
    type Listing = components["schemas"]["ProcessListing"];
    type TaskProjection = components["schemas"]["TaskProjection"];
    const projected = (
      id: string,
      agent = "lead",
      label: components["schemas"]["BackgroundTaskLabel"] = "running",
    ): TaskProjection => ({
      id,
      agent,
      status: label === "pending" || label === "starting" ? label : "running",
      kind: "command",
      name: "command " + id,
      command: "command " + id,
      turnId: "original-turn",
      created: Date.now() / 1000 - 30,
      tail: "Summary output " + id,
      display: { ...backgroundDisplay("command", "command " + id), label },
    });
    const active = projected("active");
    const child = projected("child", "worker");
    const monitor: components["schemas"]["ProcessView"] = {
      id: "approval",
      key: "approval",
      agent: "lead",
      accountKey: "default",
      epoch: 1,
      generation: "fixture",
      source: "monitor",
      status: "approval",
      name: "command approval",
      command: "command approval",
      created: Date.now() / 1000 - 30,
      bytes: 0,
      cancelRequested: false,
      outputPatterns: [],
      outputTruncated: false,
      tail: "Summary output approval",
      display: {
        ...backgroundDisplay("monitor", "command approval"),
        label: "approval",
        actions: ["readOutput", "approve", "stop"],
      },
    };
    const listing: Listing = {
      processes: [monitor],
      activities: [
        { source: "task", value: active },
        { source: "process", value: monitor },
        { source: "task", value: child },
        { source: "task", value: projected("pending", "lead", "pending") },
        { source: "task", value: projected("starting", "lead", "starting") },
        { source: "task", value: projected("stopping", "lead", "stopping") },
      ],
    };
    const otherListing: Listing = {
      processes: [],
      activities: [
        { source: "task", value: projected("other-active", "other") },
      ],
    };
    const render = async (
      leadId = "lead",
      focus?: React.ComponentProps<
        typeof import("../../src/components/shell/BackgroundTasks.tsx").default
      >["initialFocus"],
    ) => {
      currentListing = leadId === "other" ? otherListing : listing;
      await page.evaluate<void, unknown>(
        (value) => {
          const args = value as {
            data: Parameters<Window["renderFixture"]>[0];
            listing: Listing;
            leadId: string;
            focus: Parameters<Window["renderFixture"]>[3];
          };
          window.renderFixture(
            args.data,
            args.listing,
            args.leadId,
            args.focus,
          );
        },
        {
          data,
          listing: currentListing,
          leadId,
          focus,
        },
      );
    };
    const rows = () => page.locator("[data-task]");
    await render();
    await expect(rows()).toHaveCount(6);
    assert.deepEqual(
      (
        await rows().evaluateAll((nodes) =>
          nodes.map((node) => node.dataset.task),
        )
      ).sort(),
      ["active", "approval", "child", "pending", "starting", "stopping"],
    );
    assert.equal(
      await page.getByRole("radiogroup", { name: "Task status" }).count(),
      0,
    );
    assert.equal(await page.getByText("History", { exact: true }).count(), 0);
    const taskList = page.getByRole("region", {
      name: "Task list",
      exact: true,
    });
    await taskList.waitFor();
    assert.equal(
      await taskList
        .locator(".task-row-kind")
        .getByText("Command", { exact: true })
        .count(),
      5,
    );
    assert.equal(
      await taskList
        .locator(".task-row-kind")
        .getByText("Monitor", { exact: true })
        .count(),
      1,
    );
    assert.equal(
      await taskList
        .locator(".task-row-kind")
        .getByText("Tool", { exact: true })
        .count(),
      0,
    );

    assert.equal(await page.locator(".tasks-footnote").count(), 0);
    assert.equal(
      await page.getByText("Current activity", { exact: true }).count(),
      1,
    );
    const activeCommand = active;
    agents[0]!.inFlight = true;
    agents[0]!.turnId = "original-turn";
    await render();
    await page
      .locator('[data-task="active"] .task-row-kind')
      .getByText("Command", { exact: true })
      .waitFor();
    agents[0]!.turnId = "next-turn";
    active.display.kindLabel = "backgroundCommand";
    await render();
    await page
      .locator('[data-task="active"] .task-row-kind')
      .getByText("Background command", { exact: true })
      .waitFor();
    agents[0]!.turnId = "original-turn";
    agents[0]!.inFlight = false;
    await render();
    await page
      .locator('[data-task="active"] .task-row-kind')
      .getByText("Background command", { exact: true })
      .waitFor();
    active.display.kindLabel = "command";
    delete required(agents[0]).inFlight;
    delete required(agents[0]).turnId;
    await render();
    await page
      .locator('[data-task="active"] .task-row-kind')
      .getByText("Command", { exact: true })
      .waitFor();
    assert.equal(
      await page.locator('[data-task="approval"] .task-row-kind').innerText(),
      "Monitor",
    );
    await page.locator('[data-task="active"]').click();
    await page.getByRole("region", { name: "Command controls" }).waitFor();
    await page
      .locator(".task-output")
      .getByText("Summary output active", { exact: true })
      .waitFor();
    activeCommand!.tail = "Updated summary output";
    await render();
    await page
      .locator(".task-output")
      .getByText("Updated summary output", { exact: true })
      .waitFor();
    assert.equal(
      calls.filter((call) => call.path === "/api/task").length,
      0,
      "streamed task summaries issue zero idle GETs",
    );
    listing.activities = listing.activities.filter(
      (entry) => entry.value.id !== "active",
    );
    await render();
    await page.locator('[data-task="active"]').waitFor({ state: "detached" });
    assert.equal(
      await page.locator('[data-task-detail="active"]').count(),
      0,
      "Completion removes the selected command details",
    );
    await render("lead", {
      id: "completed",
      leadId: "lead",
      requestId: "historical-focus",
    });
    await page
      .getByText("The selected task is no longer active in this chat.", {
        exact: true,
      })
      .waitFor();
    assert.equal(
      await page.locator('[data-task-detail="completed"]').count(),
      0,
    );
    assert.equal(
      calls.filter((call) => call.id === "completed").length,
      0,
      "Historical focus cannot fetch old output",
    );
    await page.locator('[data-task="approval"]').click();
    await page
      .getByRole("button", { name: "Approve command", exact: true })
      .waitFor();
    await page
      .getByRole("button", { name: "Stop selected process", exact: true })
      .waitFor();
    await page.evaluate(() => {
      window.holdRefresh = true;
      window.refreshCalls = 0;
    });
    holdAnswer = true;
    const approve = page.getByRole("button", {
      name: "Approve command",
      exact: true,
    });
    const pendingControl = page.getByRole("button", {
      name: "Download task log",
      exact: true,
    });
    const answerRequest = page.waitForRequest(
      (request) =>
        request.method() === "POST" &&
        new URL(request.url()).pathname === "/api/answer",
    );
    await approve.click();
    await answerRequest;
    await expect(pendingControl).toBeDisabled();
    assert.equal(await page.evaluate(() => window.refreshCalls), 0);
    holdAnswer = false;
    const refreshed = page.evaluate(
      () =>
        new Promise<void>((resolve) => {
          if (window.refreshCalls === 1) return resolve();
          window.addEventListener("fixture-refresh", () => resolve(), {
            once: true,
          });
        }),
    );
    required(releaseAnswer)();
    await expect(pendingControl).toBeEnabled({ timeout: 2000 });
    await refreshed;
    const notified = page.evaluate(
      () =>
        new Promise<void>((resolve) => {
          if (window.notifications?.includes("Credential refresh unavailable"))
            return resolve();
          window.addEventListener("fixture-notification", () => resolve(), {
            once: true,
          });
        }),
    );
    await page.evaluate(() => {
      window.holdRefresh = false;
      window.rejectRefresh(new Error("Credential refresh unavailable"));
    });
    await notified;
    assert.equal(
      calls.filter(
        (call) => call.method === "POST" && call.path === "/api/answer",
      ).length,
      1,
    );
    await page.getByLabel("Task type", { exact: true }).selectOption("command");
    assert.equal(await page.locator('[data-task="approval"]').count(), 0);
    await page.getByLabel("Task type", { exact: true }).selectOption("all");
    await page.getByLabel("Find a task", { exact: true }).fill("child");
    assert.deepEqual(
      await rows().evaluateAll((nodes) =>
        nodes.map((node) => node.dataset.task),
      ),
      ["child"],
    );
    await page.getByLabel("Find a task", { exact: true }).fill("");
    await render("other");
    await page.locator('[data-task="other-active"]').waitFor();
    assert.deepEqual(
      await rows().evaluateAll((nodes) =>
        nodes.map((node) => node.dataset.task),
      ),
      ["other-active"],
    );
    for (const width of [320, 1920]) {
      await page.setViewportSize({ width, height: 900 });
      await render("lead", {
        id: "child",
        leadId: "lead",
        requestId: "child-focus-" + width,
      });
      await page.locator('[data-task-detail="child"]').waitFor();
      await page.getByRole("region", { name: "Command controls" }).waitFor();
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth > innerWidth,
        ),
        false,
        "No page overflow at " + width,
      );
      await page.screenshot({
        path: testInfo.outputPath("studio-current-activity-" + width + ".png"),
      });
      listing.activities = listing.activities.filter(
        (entry) => entry.value.id !== "child",
      );
      await render("lead");
      await page
        .locator('[data-task-detail="child"]')
        .waitFor({ state: "detached" });
      listing.activities.push({ source: "task", value: child });
    }
    listing.activities = [];
    listing.processes = [];
    await render("lead");
    await page.getByText("No active tasks", { exact: true }).waitFor();
    assert.equal(await rows().count(), 0);
    assert.equal(await page.locator("[data-task-detail]").count(), 0);
    assert.equal(
      calls.filter((call) => call.path === "/api/workspace").length,
      0,
      "Current activity does not fetch command history",
    );
    assert.equal(
      calls.filter((call) => call.method !== "GET").length,
      1,
      "Visibility changes cannot replay or cancel work",
    );
    assert.equal(calls.filter((call) => call.path === "/api/task").length, 0);
    console.log(JSON.stringify({ taskSummaryIdleGets: 0 }));
    assert.deepEqual(errors, []);
    console.log(
      "PASS: current snapshot only, completion hides details, approval acknowledgment releases controls before refresh, filters, chat scope, 320/1920 viewports, no history reads or duplicate writes",
    );
  } finally {
    await server.close();
    await rm(temporary, { recursive: true, force: true });
  }
});
