import { build, createServer } from "vite";
import { join } from "node:path";

export const builtActivityHtml =
  '<!doctype html><link rel="stylesheet" href="/style.css"><div id="root"></div><script type="module" src="/fixture.js"></script>';

/** Build the real activity subtree, then serve only its compiled browser assets. */
export async function builtActivityFixture(
  root: string,
  temporary: string,
  entry: string,
) {
  const outDir = join(temporary, "dist");
  await build({
    configFile: false,
    root,
    logLevel: "warn",
    define: { __STUDIO_PERF_REPORTING__: "false" },
    build: {
      target: "esnext",
      outDir,
      cssCodeSplit: false,
      rollupOptions: {
        input: join(root, "tests/background", entry),
        output: {
          entryFileNames: "fixture.js",
          assetFileNames: "[name].[ext]",
        },
      },
    },
  });
  return createServer({
    configFile: false,
    root: outDir,
    cacheDir: join(temporary, "vite"),
    server: { host: "127.0.0.1", port: 0, hmr: false },
  });
}

/** Capture settled controls together with the state that determines their colors. */
export async function captureActivity(
  page: import("@playwright/test").Page,
  testInfo: import("@playwright/test").TestInfo,
  name: string,
) {
  await page.screenshot({
    path: testInfo.outputPath(name + ".png"),
    fullPage: true,
    animations: "disabled",
  });
  const controls = await page
    .locator(".tasks-drawer button:not(.task-row)")
    .evaluateAll((buttons) =>
      buttons.map((button) => {
        const style = getComputedStyle(button);
        return {
          label:
            button.getAttribute("aria-label") || button.textContent?.trim(),
          disabled: (button as HTMLButtonElement).disabled,
          loading: button.hasAttribute("data-loading"),
          background: style.backgroundColor,
          color: style.color,
        };
      }),
    );
  await import("node:fs/promises").then(({ writeFile }) =>
    writeFile(
      testInfo.outputPath(name + ".ui-state.json"),
      JSON.stringify(controls, null, 2),
    ),
  );
}
