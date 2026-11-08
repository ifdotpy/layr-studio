import { waitForFixturePort } from "../playwright.ts";
import { fixturePostBody, fixturePostResult } from "./http.ts";
import { commandOf } from "../messages/send-command.ts";
import { test, expect, spawnFixture, rustUiFixture } from "../playwright.ts";
import { chooseSetupValue } from "../fixtures/setup-controls.ts";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

test("new chat controls are inline, save settings and keep the compact composer after send", async ({
  page,
}) => {
  test.setTimeout(120000);
  const evidence = await mkdtemp(join(tmpdir(), "studio-new-chat-settings-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, evidence], {
    stdio: ["ignore", "pipe", "pipe"],
    env: {
      FIXTURE_WORKER_COUNT: "1",
    },
  });
  let log = "";
  fixture.stderr!.on("data", (chunk) => {
    log += chunk;
  });
  try {
    const port = await waitForFixturePort(fixture, { diagnostics: () => log });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(`http://127.0.0.1:${port}`);
    await page
      .locator("[data-chat]")
      .filter({ hasText: "Release lead" })
      .first()
      .click();
    await page
      .locator("#sidebar .projects-heading")
      .getByRole("button", { name: "New chat", exact: true })
      .click();
    const creation = page.getByRole("dialog", {
      name: "New chat",
      exact: true,
    });
    await expect(
      creation.getByLabel("Main agent model", { exact: true }),
    ).toBeVisible();
    await expect(creation.getByRole("listbox")).toHaveCount(1);
    await expect(
      creation.getByText("Main agent model", { exact: true }),
    ).toHaveCount(0);
    await expect(
      creation.getByText("Main agent reasoning", { exact: true }),
    ).toHaveCount(0);
    await expect(creation.locator(".model-description")).toHaveCount(0);
    await expect(page.getByRole("dialog")).toHaveCount(1);
    const screenshotDirectory = process.env.EXECUTION_COMPACT_SCREENSHOTS;
    const measurements = [];
    if (screenshotDirectory)
      await mkdir(screenshotDirectory, { recursive: true });
    for (const [width, height] of [
      [1440, 900],
      [390, 844],
    ] as const) {
      for (const scheme of ["light", "dark"] as const) {
        await page.setViewportSize({ width, height });
        await page.emulateMedia({ colorScheme: scheme });
        await expect(page.locator("html")).toHaveAttribute(
          "data-mantine-color-scheme",
          scheme,
        );
        const geometry = await creation
          .getByRole("listbox")
          .evaluate((node) => {
            const root = getComputedStyle(document.documentElement);
            return {
              picker: node.getBoundingClientRect().width,
              dialog: node.closest('[role="dialog"]')!.getBoundingClientRect()
                .width,
              padding: parseFloat(
                getComputedStyle(node.closest(".mantine-Modal-body")!)
                  .paddingInlineStart,
              ),
              previous:
                parseFloat(root.getPropertyValue("--studio-form-max")) *
                parseFloat(root.fontSize),
              overflow: document.documentElement.scrollWidth > innerWidth,
            };
          });
        if (width === 1440) {
          expect(geometry.picker).toBeCloseTo(geometry.previous * 0.65, 0);
          expect(geometry.dialog).toBeCloseTo(
            geometry.picker + geometry.padding * 2,
            0,
          );
        }
        expect(geometry.overflow).toBe(false);
        measurements.push({ width, scheme, ...geometry });
        if (screenshotDirectory)
          await page.screenshot({
            animations: "disabled",
            path: join(screenshotDirectory, `new-chat-${width}-${scheme}.png`),
          });
      }
    }
    if (screenshotDirectory)
      await writeFile(
        join(screenshotDirectory, "new-chat-measurements.json"),
        JSON.stringify(measurements, null, 2),
      );
    await page.setViewportSize({ width: 1440, height: 900 });
    await chooseSetupValue(
      creation.getByLabel("Main agent reasoning", { exact: true }),
      "high",
    );
    const roles = creation.getByRole("radiogroup", {
      name: "Agent role",
      exact: true,
    });
    for (const name of ["Lead", "Workers", "Review"])
      await expect(roles.getByRole("radio", { name, exact: true })).toHaveCount(
        1,
      );
    await roles.getByRole("radio", { name: "Lead", exact: true }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(
      roles.getByRole("radio", { name: "Workers", exact: true }),
    ).toBeChecked();
    await page.keyboard.press("ArrowRight");
    await expect(
      roles.getByRole("radio", { name: "Review", exact: true }),
    ).toBeChecked();
    await expect(
      roles.getByRole("radio", { name: "Review", exact: true }),
    ).toBeFocused();
    const reviewModels = creation.getByRole("listbox", {
      name: "Default review model",
      exact: true,
    });
    await reviewModels
      .locator('[role="option"][data-value="gpt-6-astra"]')
      .click();
    await chooseSetupValue(
      creation.getByLabel("Default review reasoning", { exact: true }),
      "high",
    );
    await roles.getByText("Workers", { exact: true }).click();
    await expect(creation.getByRole("listbox")).toHaveCount(1);
    await expect(
      creation.getByLabel("Default subagent model", { exact: true }),
    ).toBeVisible();
    await chooseSetupValue(
      creation.getByLabel("Default subagent reasoning", { exact: true }),
      "medium",
    );
    await roles.getByText("Lead", { exact: true }).click();
    await expect(creation.getByRole("listbox")).toHaveCount(1);
    await expect(
      creation.getByLabel("Main agent reasoning", { exact: true }),
    ).toHaveAttribute("data-value", "high");
    const startChat = page.waitForResponse(
      (response) =>
        response.request().method() === "POST" &&
        response.url().endsWith("/api/leads"),
    );
    await page
      .getByRole("dialog", { name: "New chat", exact: true })
      .getByRole("button", { name: "Start chat", exact: true })
      .click();
    const createdResponse = await startChat;
    expect(createdResponse.ok()).toBe(true);
    expect(
      fixturePostBody("/api/leads", createdResponse.request().postDataJSON())
        .review_defaults,
    ).toEqual({
      model: "gpt-6-astra",
      effort: "high",
    });
    expect(
      (await fixturePostResult("/api/leads", createdResponse)).reviewDefaults,
    ).toEqual({
      model: "gpt-6-astra",
      effort: "high",
    });
    const row = page.locator("#composer .composer-context-row");
    await expect(row).toBeVisible();
    await expect(
      page
        .getByRole("navigation", { name: "Recent project chats" })
        .getByRole("button", { name: /Release lead/ }),
    ).toBeVisible();
    await expect(row.locator(".execution-menu")).toHaveCount(1);
    await expect(page.locator("#composer .execution-menu")).toHaveCount(1);
    await expect(page.locator(".empty-chat p")).toHaveCount(0);
    const trigger = row.locator(".execution-summary-trigger");
    await expect(
      trigger.locator('[data-role="lead"] .execution-effort'),
    ).toContainText("High");
    const composerBounds = await page.locator("#composer").boundingBox();
    const rowBounds = await row.boundingBox();
    const startBounds = await page.locator(".conversation-start").boundingBox();
    expect(Math.abs(startBounds!.x - composerBounds!.x)).toBeLessThanOrEqual(1);
    expect(
      Math.abs(startBounds!.width - composerBounds!.width),
    ).toBeLessThanOrEqual(1);
    expect(rowBounds!.y).toBeGreaterThanOrEqual(composerBounds!.y);
    expect(rowBounds!.y + rowBounds!.height).toBeLessThanOrEqual(
      composerBounds!.y + composerBounds!.height,
    );
    await trigger.click();
    const picker = page.locator(".execution-dropdown");
    await expect(
      picker.getByLabel("Main agent reasoning", { exact: true }),
    ).toHaveAttribute("data-value", "high");
    await picker.getByText("Workers", { exact: true }).click();
    await expect(
      page.getByLabel("Default subagent reasoning", { exact: true }),
    ).toHaveAttribute("data-value", "medium");
    await page.keyboard.press("Escape");
    await page.reload();
    await expect(
      trigger.locator('[data-role="lead"] .execution-effort'),
    ).toContainText("High");
    await page.setViewportSize({ width: 390, height: 844 });
    await expect(trigger).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Attach files", exact: true }),
    ).toBeVisible();
    expect(
      await trigger.evaluate((node) => node.scrollWidth <= node.clientWidth),
    ).toBe(true);
    await page.locator("#message").fill("Fixture first message");
    const send = page.waitForResponse(
      (response) => commandOf(response.request(), "send") !== undefined,
    );
    await page.locator("#send").click();
    expect((await send).ok()).toBe(true);
    await expect(page.locator(".conversation-start")).toHaveCount(0);
    await expect(row).toBeVisible();
    await expect(page.locator("#composer .execution-menu")).toHaveCount(1);
    const footer = page.locator(".conversation-composer-footer");
    await expect(trigger).toBeVisible();
    await expect(footer.locator(".execution-menu")).toHaveCount(0);
  } finally {
    fixture.kill("SIGTERM");
  }
});

test("confirmed new chat remains visible once while its entity projection is delayed", async ({
  page,
}) => {
  const evidence = await mkdtemp(join(tmpdir(), "studio-confirmed-creation-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, evidence], {
    stdio: ["ignore", "pipe", "pipe"],
    env: { FIXTURE_WORKER_COUNT: "1" },
  });
  let log = "";
  fixture.stderr!.on("data", (chunk) => {
    log += chunk;
  });
  const port = await waitForFixturePort(fixture, { diagnostics: () => log });
  let hold = false;
  const held: import("@playwright/test").Route[] = [];
  const firstHeld = Promise.withResolvers<void>();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.exposeFunction("creationIngressHeld", () => firstHeld.resolve());
  await page.addInitScript(() => {
    const add = EventTarget.prototype.addEventListener;
    const remove = EventTarget.prototype.removeEventListener;
    const listeners = new WeakMap<
      EventTarget,
      Map<string, Map<EventListenerOrEventListenerObject, EventListener>>
    >();
    const queued: (() => void)[] = [];
    let id: string | null = null;
    let reported = false;
    add.call(window, "fixture-creation-ingress", (event) => {
      id = (event as CustomEvent<string | null>).detail;
      if (id) reported = false;
      else queued.splice(0).forEach((deliver) => deliver());
    });
    EventTarget.prototype.addEventListener = function (
      type,
      listener,
      options,
    ) {
      if (!listener || (type !== "resources" && type !== "entity-transfer"))
        return add.call(this, type, listener, options);
      let target = listeners.get(this);
      if (!target) listeners.set(this, (target = new Map()));
      const key = `${type}:${typeof options === "boolean" ? options : !!options?.capture}`;
      let callbacks = target.get(key);
      if (!callbacks) target.set(key, (callbacks = new Map()));
      let wrapped = callbacks.get(listener);
      if (!wrapped) {
        const deliver = (event: Event) => {
          if ((this as EventSource).readyState === EventSource.CLOSED) return;
          if (typeof listener === "function") listener.call(this, event);
          else listener.handleEvent(event);
        };
        const defer = (event: Event) => {
          queued.push(() => deliver(event));
          if (!reported) {
            reported = true;
            void (
              window as typeof window & { creationIngressHeld(): Promise<void> }
            ).creationIngressHeld();
          }
        };
        wrapped = (event) => {
          if (!id) return deliver(event);
          // Transfer fragments may split the entity id; keep every fragment in order.
          if (type === "entity-transfer") return defer(event);
          const message = event as MessageEvent<string>;
          const frame: import("../../src/generated/api.ts").components["schemas"]["ResourceChangeEvent"] =
            JSON.parse(message.data);
          const entries = frame.resourceVersions.length
            ? frame.resourceVersions.map((version, index) => ({
                resource: version.resource ?? frame.resources[index]!,
                version,
              }))
            : frame.resources.map((resource) => ({
                resource,
                version: undefined,
              }));
          const held = entries.filter(
            ({ resource, version }) =>
              resource.kind === "state" ||
              version?.entityChanges?.documents.some(
                (row) => row.collection === "agentSummary" && row.id === id,
              ),
          );
          if (!held.length) return deliver(event);
          const subset = (entries: typeof held) =>
            new MessageEvent(type, {
              data: JSON.stringify({
                ...frame,
                resources: entries.map((entry) => entry.resource),
                resourceVersions: entries.flatMap((entry) =>
                  entry.version ? [entry.version] : [],
                ),
              }),
              lastEventId: message.lastEventId,
              origin: message.origin,
            });
          defer(subset(held));
          // Detail/transcript baselines still arrive so the pre-projection composer works.
          const remaining = entries.filter((entry) => !held.includes(entry));
          if (remaining.length) deliver(subset(remaining));
        };
        callbacks.set(listener, wrapped);
      }
      return add.call(this, type, wrapped, options);
    };
    EventTarget.prototype.removeEventListener = function (
      type,
      listener,
      options,
    ) {
      const key = `${type}:${typeof options === "boolean" ? options : !!options?.capture}`;
      const wrapped = listener && listeners.get(this)?.get(key)?.get(listener);
      return remove.call(this, type, wrapped || listener, options);
    };
  });
  await page.route("**/api/sync/pull?*", (route) => {
    if (hold) {
      held.push(route);
      firstHeld.resolve();
      return;
    }
    return route.continue();
  });
  await page.route("**/api/leads", async (route) => {
    hold = true;
    const id = fixturePostBody("/api/leads", route.request().postDataJSON()).id;
    if (!id) throw new Error("The create request must carry its durable id.");
    await page.evaluate(
      (id) =>
        window.dispatchEvent(
          new CustomEvent("fixture-creation-ingress", { detail: id }),
        ),
      id,
    );
    const response = await route.fetch();
    await route.fulfill({ response });
  });
  await page.goto(`http://127.0.0.1:${port}`);
  await page
    .locator("[data-chat]")
    .filter({ hasText: "Release lead" })
    .first()
    .click();
  await page
    .locator("#sidebar .projects-heading")
    .getByRole("button", { name: "New chat", exact: true })
    .click();
  const response = page.waitForResponse(
    (response) =>
      response.request().method() === "POST" &&
      response.url().endsWith("/api/leads"),
  );
  await page
    .getByRole("dialog", { name: "New chat", exact: true })
    .getByRole("button", { name: "Start chat", exact: true })
    .click();
  const receipt = await fixturePostResult("/api/leads", await response);
  await firstHeld.promise;
  await page.waitForFunction(
    (id) =>
      JSON.parse(
        localStorage.getItem("codex-confirmed-chats") || "null",
      )?.value?.agents.some((agent: { id: string }) => agent.id === id),
    receipt.id,
  );
  const row = page.locator(`[data-chat="${receipt.id}"]`);
  await expect(row).toHaveCount(1);
  await expect(row).toHaveAttribute("aria-current", "true");
  await expect(page.locator("#conversation-title")).toHaveText("New chat");
  await expect(page.locator("#message")).toBeEnabled();
  await page.screenshot({
    path: join(evidence, "confirmed-before-projection.png"),
    animations: "disabled",
  });
  hold = false;
  await page.evaluate(() =>
    window.dispatchEvent(
      new CustomEvent("fixture-creation-ingress", { detail: null }),
    ),
  );
  await Promise.all(held.splice(0).map((route) => route.continue()));
  await page.waitForFunction((id) => {
    const value = JSON.parse(
      localStorage.getItem("codex-confirmed-chats") || "null",
    )?.value;
    return (
      value && !value.agents.some((agent: { id: string }) => agent.id === id)
    );
  }, receipt.id);
  await expect(row).toHaveCount(1);
  await expect(row).toHaveAttribute("aria-current", "true");
  await page.screenshot({
    path: join(evidence, "confirmed-after-projection.png"),
    animations: "disabled",
  });
  expect(errors).toEqual([]);
});
