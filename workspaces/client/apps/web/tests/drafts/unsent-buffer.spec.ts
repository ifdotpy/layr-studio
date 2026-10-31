import type { AddressInfo } from "node:net";
import { fileURLToPath } from "node:url";
import { test, expect, createFixtureViteServer } from "../playwright.ts";

test("unconfirmed draft survives reload and disappears after confirmation", async ({
  page,
}) => {
  const server = await createFixtureViteServer({
    configFile: false,
    root: fileURLToPath(new URL("../../", import.meta.url)),
    server: { host: "127.0.0.1", port: 0 },
    plugins: [
      {
        name: "unsent-fixture",
        configureServer(vite) {
          vite.middlewares.use("/unsent-check", (_request, response) => {
            response.setHeader("Content-Type", "text/html");
            response.end(
              '<div id="root"></div><script type="module" src="/tests/drafts/unsent-buffer.fixture.tsx"></script>',
            );
          });
        },
      },
    ],
  });
  await server.listen();
  try {
    const address = server.httpServer?.address();
    if (!address || typeof address === "string")
      throw new Error("Fixture server has no port");
    await page.goto(
      `http://127.0.0.1:${(address satisfies AddressInfo).port}/unsent-check`,
    );
    await expect(page.locator("output")).toHaveText("ready");
    await page.getByLabel("Draft").fill("keep this draft");
    await expect(page.locator("output")).toHaveText("saved");
    await page.reload();
    await expect(page.getByLabel("Draft")).toHaveValue("keep this draft");
    await page.getByRole("button", { name: "Confirm" }).click();
    await expect(page.locator("output")).toHaveText("confirmed");
    await page.reload();
    await expect(page.locator("output")).toHaveText("ready");
    await expect(page.getByLabel("Draft")).toHaveValue("");
  } finally {
    await server.close();
  }
});
