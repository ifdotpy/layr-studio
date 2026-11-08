import { fixtureOrigin } from "../../../../../tests/misc-fixture.ts";
// Mount the production input by itself: no App or Conversation wrapper.
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import type { ViteDevServer } from "vite";
import { expect, test } from "../../../../../tests/playwright.ts";

declare global {
  interface Window {
    imeWitness?: {
      input: HTMLTextAreaElement;
      writes: string[];
      compositionEnds: number;
    };
  }
}

let cacheDir: string;
let server: ViteDevServer;

test.beforeAll(async () => {
  cacheDir = await mkdtemp(join(tmpdir(), "studio-prompt-input-test-"));
  server = await createServer({
    configFile: false,
    root: fileURLToPath(new URL("../../../../..", import.meta.url)),
    cacheDir,
    optimizeDeps: {
      noDiscovery: true,
      include: [
        "react",
        "react/jsx-runtime",
        "react/jsx-dev-runtime",
        "react-dom/client",
        "react-dom",
        "@mantine/core",
      ],
    },
    server: { host: "127.0.0.1", port: 0, hmr: false },
  });
  await server.listen();
});

test("isolated production PromptInput preserves its input behavior", async ({
  page,
}) => {
  const errors: string[] = [];
  page.setDefaultTimeout(10000);
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  page.on("pageerror", (error) => errors.push(error.message));

  await page.route("**/api/skills?*", (route) =>
    route.fulfill({
      json: {
        skills: [
          { name: "eli5", description: "Simple explanation", path: "/eli5" },
          { name: "doc1", description: "Durable docs", path: "/doc1" },
        ],
        errors: [],
      },
    }),
  );
  await page.route("**/prompt-input-test", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: "<!doctype html><html><head></head><body><div id='root'></div></body></html>",
    }),
  );
  await page.goto(`${fixtureOrigin(server)}/prompt-input-test`);
  await page.evaluate(async () => {
    const { mount } =
      await import("/src/components/conversation/prompt-composer/PromptInput/PromptInput.test-entry.tsx");
    mount();
  });
  await page
    .locator("#message")
    .waitFor()
    .catch(async () => {
      throw new Error(
        `isolated PromptInput did not mount: ${await page.locator("body").innerHTML()} ${errors.join("\n")}`,
      );
    });

  const input = page.getByRole("combobox", { name: "Message" });
  await page.evaluate(() => {
    const descriptor = Object.getOwnPropertyDescriptor(
      Element.prototype,
      "scrollHeight",
    );
    if (!descriptor?.get) throw new Error("scrollHeight getter is missing");
    let reads = 0;
    let inputReads = 0;
    Object.defineProperty(Element.prototype, "scrollHeight", {
      ...descriptor,
      get() {
        if (this.getAttribute("aria-label") === "autosize measurement") reads++;
        if (this.id === "message") inputReads++;
        return descriptor.get!.call(this);
      },
    });
    Object.defineProperty(window, "composerMeasurementReads", {
      get: () => reads,
    });
    Object.defineProperty(window, "composerInputReads", {
      get: () => inputReads,
    });
  });
  await input.fill("hello");
  await expect(input).toHaveValue("hello");
  expect(
    await page.evaluate(() =>
      Object.getOwnPropertyDescriptor(
        window,
        "composerMeasurementReads",
      )?.get?.(),
    ),
  ).toBe(0);
  await expect
    .poll(() => page.evaluate(() => window.inputEvents!.changes))
    .toBe(1);

  const oneLine = await input.evaluate(
    (element) => element.getBoundingClientRect().height,
  );
  const beforeResize = await page.evaluate(() =>
    Object.getOwnPropertyDescriptor(window, "composerInputReads")?.get?.(),
  );
  await page.evaluate(() => {
    window.setInputState!({ text: "one\ntwo\nthree\nfour" });
    window.setInputState!({ text: "one\ntwo\nthree\nfour\nfive" });
  });
  await expect
    .poll(() =>
      input.evaluate((element) => element.getBoundingClientRect().height),
    )
    .toBeGreaterThan(oneLine);
  const afterResize = await page.evaluate(() =>
    Object.getOwnPropertyDescriptor(window, "composerInputReads")?.get?.(),
  );
  expect(afterResize - beforeResize).toBe(1);
  await page.evaluate(() => window.setInputState!({ text: "hello" }));
  await expect
    .poll(() =>
      input.evaluate((element) => element.getBoundingClientRect().height),
    )
    .toBeLessThanOrEqual(oneLine);

  await input.press("Enter");
  await expect
    .poll(() => page.evaluate(() => window.inputEvents!.sends))
    .toBe(1);
  await input.press("Shift+Enter");
  await expect
    .poll(() => page.evaluate(() => window.inputEvents!.sends))
    .toBe(1);
  await page.evaluate(() => {
    const input = document.querySelector<HTMLTextAreaElement>("#message");

    input!.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Enter",
        bubbles: true,
        isComposing: true,
      }),
    );
  });
  await expect
    .poll(() => page.evaluate(() => window.inputEvents!.sends))
    .toBe(1);

  await input.press("Tab");
  await expect
    .poll(() => page.evaluate(() => window.inputEvents!.queues))
    .toBe(1);
  await page.evaluate(() => window.setInputState!({ canSend: false }));
  await expect(input).toBeDisabled();
  const changesBeforeDisabledInput = await page.evaluate(
    () => window.inputEvents!.changes,
  );
  await input.press("x").catch(() => {});
  await expect
    .poll(() => page.evaluate(() => window.inputEvents!.changes))
    .toBe(changesBeforeDisabledInput);

  await page.evaluate(() =>
    window.setInputState!({
      canSend: true,
      text: "x".repeat(12001),
      tooLong: true,
    }),
  );
  await expect(input).toHaveAttribute("aria-invalid", "true");
  await input.evaluate((element) => {
    element.style.maxHeight = "80px";
    element.style.boxSizing = "border-box";
    element.style.overflowY = "auto";
  });
  await expect
    .poll(() =>
      input.evaluate((element) => element.scrollHeight > element.clientHeight),
    )
    .toBe(true);
  expect(
    await input.evaluate((element) => element.getBoundingClientRect().height),
  ).toBeLessThanOrEqual(80);

  await page.evaluate(() => {
    window.setInputState!({ text: "$eli5", tooLong: false });
    const element = document.querySelector<HTMLTextAreaElement>("#message");

    element!.setSelectionRange(element!.value.length, element!.value.length);
    element!.dispatchEvent(new Event("select", { bubbles: true }));
  });
  await page.getByRole("option", { name: /eli5/ }).waitFor();
  await input.press("Enter");
  await expect(input).toHaveValue("$eli5 ");
  await expect
    .poll(() => page.evaluate(() => window.inputEvents!.sends))
    .toBe(1);
  expect(errors).toEqual([]);
});

test.afterAll(async () => {
  await server?.close();
  if (cacheDir) await rm(cacheDir, { recursive: true, force: true });
});

test("touch inputs keep the focus zoom font floor across layout widths", async ({
  browser,
}) => {
  for (const hasTouch of [true, false]) {
    const context = await browser.newContext({ hasTouch });
    try {
      const page = await context.newPage();
      await page.route("**/touch-input-test", (route) =>
        route.fulfill({
          contentType: "text/html",
          body: `<!doctype html><meta name="viewport" content="width=device-width, initial-scale=1">
            <div class="workspace">
              <aside id="sidebar"><input id="chat-search"></aside>
              <main id="conversation"><form id="composer"><div id="root"></div></form></main>
              <div id="controls"></div>
            </div>`,
        }),
      );
      await page.goto(`${fixtureOrigin(server)}/touch-input-test`);
      await page.evaluate(async () => {
        await import("/node_modules/@mantine/core/styles.css");
        await import("/src/style.css");
        await import("/src/studio-preferences.css");
        await import("/src/components/shell/Analytics/Analytics.css");
        const { mount, mountControls } =
          await import("/src/components/conversation/prompt-composer/PromptInput/PromptInput.test-entry.tsx");
        mount();
        mountControls();
      });
      await page.locator("#message").waitFor({ state: "attached" });
      await page.locator("#form-select").waitFor({ state: "attached" });
      for (const width of [390, 760, 761, 844, 1024, 1440]) {
        await page.setViewportSize({ width, height: 900 });
        for (const typography of ["original", "custom"]) {
          for (const fontSize of [12, 18, 24]) {
            await page.evaluate(
              ({ typography, fontSize }) => {
                const root = document.documentElement;
                root.dataset.studioTypography = typography;
                root.style.setProperty(
                  "--studio-main-font-size",
                  `${fontSize}px`,
                );
                root.style.setProperty(
                  "--studio-sidebar-font-size",
                  `${fontSize}px`,
                );
              },
              { typography, fontSize },
            );
            const floor = hasTouch || width <= 760;
            for (const id of [
              "message",
              "chat-search",
              "form-input",
              "form-textarea",
              "form-select",
              "worker-search",
              "analytics-select",
            ]) {
              const actual = await page
                .locator(`#${id}`)
                .evaluate((node) =>
                  parseFloat(getComputedStyle(node).fontSize),
                );
              const preferred =
                typography === "custom" &&
                ["message", "chat-search"].includes(id)
                  ? fontSize
                  : id === "message"
                    ? 14
                    : 12;
              expect(
                actual,
                JSON.stringify({ id, width, hasTouch, typography, fontSize }),
              ).toBe(floor ? Math.max(16, preferred) : preferred);
            }
          }
        }
      }
    } finally {
      await context.close();
    }
  }
});

for (const mobile of [true, false])
  for (const [external, laterNative, reset] of [
    ["Incoming draft during composition", false, false],
    ["Incoming draft during composition", true, false],
    ["Restored draft", false, false],
    ["Restored draft", true, false],
    ["", true, false],
    ["", false, true],
    ["", true, true],
  ] as const)
    test(`composition orders edits for ${JSON.stringify(external)} (mobile=${mobile}, laterNative=${laterNative}, reset=${reset})`, async ({
      browser,
    }) => {
      const page = await browser.newPage({
        viewport: { width: 390, height: 844 },
        isMobile: true,
        hasTouch: true,
      });
      try {
        await page.route("**/prompt-input-ime", (route) =>
          route.fulfill({
            contentType: "text/html",
            body: '<!doctype html><meta name="viewport" content="width=device-width, initial-scale=1"><form id="composer"><div id="root"></div></form>',
          }),
        );
        await page.goto(`${fixtureOrigin(server)}/prompt-input-ime`);
        await page.evaluate(async (mobile) => {
          const { mount } =
            await import("/src/components/conversation/prompt-composer/PromptInput/PromptInput.test-entry.tsx");
          mount(mobile);
          const { attachMobileViewport } =
            await import("/src/hooks/mobileViewport.ts");
          attachMobileViewport();
        }, mobile);
        const input = page.locator("#message");
        await input.fill("Draft");
        await input.focus();
        await input.evaluate(async (node) => {
          if (!(node instanceof HTMLTextAreaElement))
            throw new Error("Missing textarea");
          await new Promise<void>((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
          );
          const writes: string[] = [];
          const backingText = new MutationObserver(() =>
            writes.push("backing text"),
          );
          backingText.observe(node, {
            childList: true,
            characterData: true,
            subtree: true,
          });
          for (const name of [
            "value",
            "selectionStart",
            "selectionEnd",
          ] as const) {
            const descriptor = Object.getOwnPropertyDescriptor(
              HTMLTextAreaElement.prototype,
              name,
            );
            if (!descriptor?.set || !descriptor.get) throw new Error(name);
            Object.defineProperty(node, name, {
              configurable: true,
              get: () => descriptor.get!.call(node),
              set: (value) => {
                writes.push(name);
                descriptor.set!.call(node, value);
              },
            });
          }
          for (const name of ["focus", "blur", "setSelectionRange"] as const) {
            const original = node[name].bind(node);
            Object.defineProperty(node, name, {
              configurable: true,
              value: (...args: unknown[]) => {
                writes.push(name);
                return Reflect.apply(original, node, args);
              },
            });
          }
          const probe = { input: node, writes, compositionEnds: 0 };
          window.imeWitness = probe;
          node.addEventListener(
            "compositionend",
            () => probe.compositionEnds++,
          );
        });
        const cdp = await page.context().newCDPSession(page);
        await cdp.send("Input.imeSetComposition", {
          text: "編",
          selectionStart: 1,
          selectionEnd: 1,
        });
        await expect(input).toHaveValue("Draft編");
        await page.setViewportSize({ width: 390, height: 520 });
        await page.evaluate(
          ({ external, reset }) => {
            window.visualViewport?.dispatchEvent(new Event("resize"));
            window.dispatchEvent(new Event("resize"));
            window.setInputState!({
              tooLong: true,
              text: external,
              reset,
            });
          },
          { external, reset },
        );
        await expect(input).toHaveAttribute("aria-invalid", "true");
        expect(
          await input.evaluate((node) => {
            const probe = window.imeWitness;
            if (!probe) throw new Error("Missing IME witness");
            return {
              same: node === probe.input,
              focused: document.activeElement === node,
              writes: probe.writes,
              compositionEnds: probe.compositionEnds,
            };
          }),
        ).toEqual({
          same: true,
          focused: true,
          writes: [],
          compositionEnds: 0,
        });
        if (laterNative) {
          await cdp.send("Input.imeSetComposition", {
            text: "編集",
            selectionStart: 2,
            selectionEnd: 2,
          });
          await expect(input).toHaveValue("Draft編集");
        }
        await cdp.send("Input.insertText", {
          text: laterNative ? "編集" : "編",
        });
        await expect(input).toHaveValue(
          laterNative && !reset ? "Draft編集" : external,
        );
        expect(
          await page.evaluate(() => window.imeWitness?.compositionEnds),
        ).toBe(1);
        await page.evaluate(() =>
          window.setInputState!({ text: "Restored draft", tooLong: false }),
        );
        await expect(input).toHaveValue("Restored draft");
      } finally {
        await page.close();
      }
    });

for (const mobile of [true, false])
  for (const beforeSelection of [true, false])
    test(`autocomplete leaves composition focus and selection alone (mobile=${mobile}, beforeSelection=${beforeSelection})`, async ({
      page,
    }) => {
      await page.route("**/prompt-autocomplete-ime", (route) =>
        route.fulfill({
          contentType: "text/html",
          body: '<!doctype html><div id="root"></div>',
        }),
      );
      await page.goto(`${fixtureOrigin(server)}/prompt-autocomplete-ime`);
      await page.evaluate(async (mobile) => {
        const { mount } =
          await import("/src/components/conversation/prompt-composer/PromptInput/PromptInput.test-entry.tsx");
        mount(mobile);
      }, mobile);
      const input = page.locator("#message");
      await input.fill("/ren");
      await expect(page.getByRole("option", { name: /rename/ })).toBeVisible();
      expect(
        await page.evaluate(async (beforeSelection) => {
          const input = document.querySelector<HTMLTextAreaElement>("#message");
          const option = document.querySelector<HTMLElement>('[role="option"]');
          if (!input || !option)
            throw new Error("Missing autocomplete input or option");
          const writes: string[] = [];
          for (const name of ["focus", "setSelectionRange"] as const) {
            const original = input[name].bind(input);
            Object.defineProperty(input, name, {
              configurable: true,
              value: (...args: unknown[]) => {
                writes.push(name);
                return Reflect.apply(original, input, args);
              },
            });
          }
          const start = () =>
            input.dispatchEvent(
              new CompositionEvent("compositionstart", { bubbles: true }),
            );
          if (beforeSelection) start();
          option.click();
          if (!beforeSelection) start();
          await new Promise<void>((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
          );
          return writes;
        }, beforeSelection),
      ).toEqual([]);
      await input.dispatchEvent("compositionend");
      await expect(input).toHaveValue("/rename ");
    });
