import type {} from "./unified-agent-picker-browser.fixture.tsx";
import { fixtureUrl } from "./fixture-helpers.ts";
import {
  apiSchemaHandshakeSse,
  API_SCHEMA_HASH_HEADER,
  readApiSchemaHash,
  test,
  expect,
  createFixtureViteServer,
} from "../playwright.ts";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

test("workers can choose a Claude model with a Codex lead", async ({
  browser,
}) => {
  const cacheDir = await mkdtemp(join(tmpdir(), "studio-worker-provider-"));
  const server = await createFixtureViteServer({
    configFile: false,
    root: join(import.meta.dirname, "../../../web"),
    cacheDir,
    server: { host: "127.0.0.1", port: 0 },
    optimizeDeps: {
      noDiscovery: true,
      include: [
        "react",
        "react/jsx-runtime",
        "react/jsx-dev-runtime",
        "react-dom/client",
        "react-dom",
        "@mantine/core",
        "@mantine/hooks",
        "lucide-react",
      ],
    },
    plugins: [
      {
        name: "worker-provider-fixture",
        configureServer(vite) {
          vite.middlewares.use("/check", (_req, res) => {
            res.setHeader("Content-Type", "text/html");
            res.end(
              '<div id="root"></div><script type="module" src="/tests/settings/unified-agent-picker-browser.fixture.tsx"></script>',
            );
          });
        },
      },
    ],
  });
  const context = await browser.newContext();
  try {
    await server.listen();
    const page = await context.newPage();
    page.setDefaultTimeout(5000);
    await page.route("**/api/sync/stream?**", (route) =>
      route.fulfill({
        contentType: "text/event-stream",
        body: apiSchemaHandshakeSse(""),
      }),
    );
    await page.route("**/api/sync/identity", (route) =>
      route.fulfill({
        headers: { [API_SCHEMA_HASH_HEADER]: readApiSchemaHash() },
        json: {
          workspaceId: "1234567890abcdef1234567890abcdef",
          syncProtocol: 2,
        },
      }),
    );
    await page.goto(fixtureUrl(server, "check"));
    await page.waitForFunction(() => !!window.pickerReset);
    await page
      .getByRole("button", { name: "Main agent settings", exact: true })
      .click();
    await page.getByText("Workers", { exact: true }).click();
    await page
      .getByLabel("Subagent account provider", { exact: true })
      .locator("label")
      .filter({ hasText: "Claude" })
      .click();
    await expect(
      page.getByLabel("Exclude Personal 2 from workers and reviewers"),
    ).toBeVisible();
    const option = page.getByRole("option", {
      name: "Sonnet 5.5",
      exact: true,
    });
    await expect(option).toBeVisible();
    await option.click();
    await page.waitForFunction(() =>
      window.pickerCalls.some(
        (call) => call.worker_defaults?.model === "claude-sonnet-5-5",
      ),
    );
  } finally {
    await context.close();
    await server.close();
    await rm(cacheDir, { recursive: true, force: true });
  }
});
