import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Page, Route } from "@playwright/test";
import type { PromptComposerRenderProbe } from "../../src/components/prompt-composer/renderProbe.ts";
import { commandOf, sendReply } from "../messages/send-command.ts";
import type { FixtureStep } from "../fixtures/fixture-scripts.ts";
import type { PostBody } from "../../src/api.ts";
import { randomUUID } from "../../src/uuid.ts";
import {
  test,
  expect,
  spawnFixture,
  rustUiFixture,
  waitForFixturePort,
  readTestState,
} from "../playwright.ts";

declare global {
  interface Window {
    composerIsolation?: {
      input: HTMLTextAreaElement;
      attachment?: HTMLElement;
      renders: Partial<
        Record<Parameters<PromptComposerRenderProbe>[0], number>
      >;
      changed: number;
    };
  }
}

async function withComposer(
  page: Page,
  run: (fixture: {
    page: Page;
    state: Awaited<ReturnType<typeof readTestState>>;
    updateModel(): Promise<void>;
    agentId: string;
    control(steps: FixtureStep[]): void;
  }) => Promise<void>,
  scripts?: { prompt: string; steps: FixtureStep[] }[],
) {
  const directory = await mkdtemp(join(tmpdir(), "studio-composer-isolation-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, directory], {
    stdio: ["pipe", "pipe", "pipe"],
    env: {
      ...process.env,
      ...(scripts ? { FIXTURE_SCRIPT: JSON.stringify(scripts) } : {}),
    },
  });
  let log = "";
  fixture.stderr!.on("data", (chunk) => (log += chunk));
  try {
    const origin = `http://127.0.0.1:${await waitForFixturePort(fixture, { diagnostics: () => log })}`;
    const state = await readTestState(origin, { agentDetails: true });
    const agent = state.threads.find((entry) => entry.name === "Release lead");
    if (!agent) throw new Error("Missing fixture lead");
    const session = Promise.withResolvers<void>();
    await page.route("**/api/session", async (route) => {
      await session.promise;
      await route.continue();
    });
    await page.goto(origin, { waitUntil: "domcontentloaded" });
    await expect(
      page.getByRole("main", { name: "Studio startup" }),
    ).toBeVisible();
    session.resolve();
    await page.locator("[data-chat]", { hasText: "Release lead" }).click();
    await expect(page.locator("#message")).toBeVisible();
    await expect(page.locator("#messages .message").first()).toBeVisible();
    await run({
      page,
      state,
      agentId: agent.id,
      control: (steps) => {
        fixture.stdin?.write(
          JSON.stringify({
            method: "fixture/provider",
            agent: agent.id,
            steps,
          }) + "\n",
        );
      },
      updateModel: async () => {
        const response = await fetch(`${origin}/api/conversation`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-Canvas-Token": state.token,
            Origin: origin,
          },
          body: JSON.stringify({
            id: agent.id,
            model: "gpt-6-luna",
            effort: "medium",
            request_id: randomUUID(),
          } satisfies PostBody<"/api/conversation">),
        });
        expect(response.ok, await response.text()).toBe(true);
      },
    });
  } finally {
    fixture.kill("SIGTERM");
  }
}

test("App crosses its loading exit into the active conversation without changing hook order", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await withComposer(page, async () => {
    await expect(page.locator("#conversation-title")).toContainText(
      "Release lead",
    );
    await expect(page.locator("#message")).toBeEnabled();
  });
  expect(errors).toEqual([]);
});

test("App composer typing leaves the application and transcript idle and preserves selection during model updates", async ({
  page,
}) => {
  test.setTimeout(120_000);
  await withComposer(page, async ({ updateModel }) => {
    const input = page.locator("#message");
    await input.fill("Draft ");
    await page
      .locator('.composer-tools input[type="file"]')
      .setInputFiles({
        name: "isolation-notes.txt",
        mimeType: "text/plain",
        buffer: Buffer.from("Keep this attachment through model updates."),
      });
    await expect(
      page.locator(".composer-tools .attachment-list"),
    ).toContainText("isolation-notes.txt");
    await input.focus();
    await page.evaluate(async () => {
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      );
      const input = document.querySelector<HTMLTextAreaElement>("#message");
      if (!input) throw new Error("Missing composer");
      const attachment =
        document.querySelector<HTMLElement>(".attachment-chip");
      if (!attachment) throw new Error("Missing saved attachment");
      const probe = {
        input,
        attachment,
        renders: {},
        changed: performance.now(),
      };
      window.composerIsolation = probe;
      window.__studioPromptComposerRenderProbe = (component) => {
        const renders = window.composerIsolation!.renders;
        renders[component] = (renders[component] || 0) + 1;
        window.composerIsolation!.changed = performance.now();
      };
    });
    // Start after initial detail/account/draft acquisitions have committed.
    await page.waitForFunction(
      () => performance.now() - window.composerIsolation!.changed > 300,
    );
    await page.evaluate(() => {
      window.composerIsolation!.renders = {};
    });
    await input.pressSequentially("typing stays local");
    const renders = await page.evaluate(
      () => window.composerIsolation!.renders,
    );
    expect(renders["prompt-composer"]).toBeGreaterThan(0);
    for (const key of [
      "app",
      "conversation",
      "turn-history",
      "message",
      "history-row",
    ] as const)
      expect(renders[key] || 0, key).toBe(0);
    await input.evaluate((element) => {
      const textarea = element as HTMLTextAreaElement;
      textarea.setSelectionRange(3, 8);
      textarea.dispatchEvent(
        new CompositionEvent("compositionstart", { bubbles: true, data: "編" }),
      );
    });
    await updateModel();
    // Positive control: the visible model summary consumes the changed fact.
    await expect(
      page.locator(".execution-summary-trigger [data-role=lead]"),
    ).toContainText(/Luna/);
    expect(
      await input.evaluate((element) => {
        const textarea = element as HTMLTextAreaElement;
        return [
          element === window.composerIsolation?.input,
          document.activeElement === element,
          textarea.selectionStart,
          textarea.selectionEnd,
        ];
      }),
    ).toEqual([true, true, 3, 8]);
    await expect(
      page.locator(".composer-tools .attachment-list"),
    ).toContainText("isolation-notes.txt");
    expect(
      await page
        .locator(".attachment-chip")
        .evaluate((node) => node === window.composerIsolation?.attachment),
    ).toBe(true);
    await input.dispatchEvent("compositionend", { data: "編" });
  });
});

test("App retries the first lost send unchanged while a newer draft survives", async ({
  page,
}) => {
  test.setTimeout(120_000);
  await withComposer(page, async () => {
    const attempts: string[] = [];
    const first = Promise.withResolvers<Route>();
    await page.route("**/api/commands", async (route) => {
      const request = commandOf(route.request(), "send");
      if (!request) return route.fallback();
      attempts.push(JSON.stringify(request));
      if (attempts.length === 1) first.resolve(route);
      else
        await route.fulfill({
          json: sendReply({ id: request.id, status: "delivered" }),
        });
    });
    const input = page.locator("#message");
    await input.fill("First durable payload");
    await page.locator("#send").click();
    const lost = await first.promise;
    await expect(input).toHaveValue("");
    const draftSaved = page.waitForResponse(
      (response) =>
        response.ok() &&
        response.url().endsWith("/api/sync/drafts") &&
        !!response.request().postData()?.includes("A newer unsent draft"),
    );
    await input.fill("A newer unsent draft");
    await lost.abort("connectionfailed");
    await page
      .getByRole("button", { name: "Retry", exact: true })
      .first()
      .click();
    await expect.poll(() => attempts.length).toBe(2);
    expect(attempts[1]).toBe(attempts[0]);
    await expect(input).toHaveValue("A newer unsent draft");
    await draftSaved;
    await page.reload();
    await page.locator("[data-chat]", { hasText: "Release lead" }).click();
    await expect(input).toHaveValue("A newer unsent draft");
    expect(attempts).toHaveLength(2);
  });
});

test("100 separately delivered transcript updates leave the production App and header idle", async ({
  page,
}, testInfo) => {
  test.setTimeout(120_000);
  await withComposer(
    page,
    async ({ control }) => {
      await page.locator("#message").fill("App token isolation stream");
      await page.locator("#send").click();
      const stream = page.locator('[data-message$=":app-isolation-stream"]');
      await expect(stream).toContainText("start");
      await page.evaluate(() => {
        const input = document.querySelector<HTMLTextAreaElement>("#message");
        if (!input) throw new Error("Missing composer");
        window.composerIsolation = {
          input,
          renders: {},
          changed: performance.now(),
        };
        window.__studioPromptComposerRenderProbe = (kind) => {
          const probe = window.composerIsolation!;
          probe.renders[kind] = (probe.renders[kind] || 0) + 1;
          probe.changed = performance.now();
        };
      });
      await page.waitForFunction(
        () => performance.now() - window.composerIsolation!.changed > 300,
      );
      await page.evaluate(() => {
        window.composerIsolation!.renders = {};
      });
      const delivered: string[] = [];
      for (let index = 0; index < 100; index++) {
        const marker = `token-${index}-end`;
        control([
          { item: "app-isolation-stream", delta: ` ${marker}`, append: true },
        ]);
        // The next delta is issued only after this intermediate state is rendered.
        await expect(stream).toContainText(marker);
        delivered.push(await stream.innerText());
      }
      expect(new Set(delivered).size).toBe(100);
      const renders = await page.evaluate(
        () => window.composerIsolation!.renders,
      );
      const report = {
        deliveredUpdates: delivered.length,
        renders,
        appRenders: renders.app || 0,
        appRendersPerUpdate: (renders.app || 0) / delivered.length,
      };
      await testInfo.attach("app-transcript-isolation", {
        body: JSON.stringify(report, null, 2),
        contentType: "application/json",
      });
      console.log(`App transcript isolation ${JSON.stringify(report)}`);
      expect(
        await page
          .locator("#message")
          .evaluate((input) => input === window.composerIsolation!.input),
      ).toBe(true);
      if (process.env.STUDIO_TRANSCRIPT_BASELINE !== "1") {
        for (const key of [
          "app",
          "conversation",
          "conversation-header",
          "settings",
          "sidebar",
        ] as const)
          expect(renders[key] || 0, key).toBe(0);
      }
    },
    [
      {
        prompt: "App token isolation stream",
        steps: [
          { item: "app-isolation-stream", delta: "start " },
          { hold: true },
        ],
      },
    ],
  );
});
