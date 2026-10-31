import type {} from "./unsent-buffer.fixture.tsx";
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
    const second = await page.context().newPage();
    await second.goto(page.url());
    await expect(second.locator("output")).toHaveText("ready");
    await page.evaluate(() => window.unsentFixture.put("race", "submitted"));
    await second.evaluate(() => window.unsentFixture.put("race", "newer edit"));
    await page.evaluate(() =>
      window.unsentFixture.deleteIfEqual("race", "submitted"),
    );
    expect(
      await second.evaluate(
        async () =>
          (await window.unsentFixture.loadAll()).find(
            ([key]) => key === "race",
          )?.[1],
      ),
    ).toBe("newer edit");
    await page.getByLabel("Form").fill("submitted form");
    await expect
      .poll(async () =>
        page.evaluate(
          async () =>
            (await window.unsentFormFixture.loadAll()).find(
              ([key]) => key === "review",
            )?.[1],
        ),
      )
      .toBe("submitted form");
    await page.getByRole("button", { name: "Begin request" }).click();
    await second.getByLabel("Form").fill("newer form");
    await expect
      .poll(async () =>
        second.evaluate(
          async () =>
            (await window.unsentFormFixture.loadAll()).find(
              ([key]) => key === "review",
            )?.[1],
        ),
      )
      .toBe("newer form");
    await page.getByRole("button", { name: "Refuse" }).click();
    await expect(page.getByText("Refusal processed")).toBeVisible();
    await expect
      .poll(async () =>
        second.evaluate(
          async () =>
            (await window.unsentFormFixture.loadAll()).find(
              ([key]) => key === "review",
            )?.[1],
        ),
      )
      .toBe("newer form");
    await second.reload();
    await expect(second.getByLabel("Form")).toHaveValue("newer form");
    await second.close();
  } finally {
    await server.close();
  }
});
