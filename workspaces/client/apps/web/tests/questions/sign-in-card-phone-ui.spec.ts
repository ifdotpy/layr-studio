import type {} from "./sign-in-card-phone-ui.fixture.tsx";
import { createBuiltQuestionServer } from "./builtQuestionServer";
import type { AddressInfo } from "node:net";
import { phoneSignInUrl } from "./sign-in.fixture.ts";

// A sign-in card at phone width. The window has no
// desktop bridge and is not on the server's own Mac, so it opens the link in paste mode,
// then sends the pasted address of the page that did not load once. A device code card
// shows the code with a copy button. The card fits the screen without a horizontal scroll.

import { createServer as createNetServer } from "node:net";
import { test, expect } from "../playwright.ts";
import { browserExecutablePath } from "../browser-executable.ts";
import {
  answerReply,
  answerRequest,
  isAnswerRequest,
  type AnswerRequest,
} from "../answer-command";

const PASTED = "http://127.0.0.1:40645/oauth/callback?code=c1&state=s1";
// A host name that is not a loopback name: the window acts as a phone on the tailnet. The
// browser resolves it to the local Vite server.
const PHONE_HOST = "studio-phone.test";
// A phone reaches Studio over https, a secure context. The test page is plain http, so the
// browser treats its origin as secure; that origin needs its port before the launch.
const PORT = await new Promise<number>((resolve, reject) => {
  const probe = createNetServer();
  probe.once("error", reject);
  probe.listen(0, "127.0.0.1", () => {
    const { port } = probe.address() as AddressInfo;
    probe.close(() => resolve(port));
  });
});
const ORIGIN = `http://${PHONE_HOST}:${PORT}`;

test.use({
  viewport: { width: 390, height: 844 },
  hasTouch: true,
  launchOptions: {
    executablePath: browserExecutablePath,
    args: [
      `--host-resolver-rules=MAP ${PHONE_HOST} 127.0.0.1`,
      `--unsafely-treat-insecure-origin-as-secure=${ORIGIN}`,
    ],
  },
});

test("Sign-in card at phone width uses paste mode and the device code", async ({
  page,
}, testInfo) => {
  const server = await createBuiltQuestionServer(
    "./sign-in-card-phone-ui.fixture.tsx",
    { port: PORT },
  );
  await server.listen();
  try {
    page.setDefaultTimeout(5000);
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    await page.addInitScript((pasted) => {
      window.__opened = [];
      window.__copied = [];
      window.open = (url) => {
        window.__opened.push(String(url));
        return {} as Window;
      };
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: {
          writeText: async (text: string) => window.__copied.push(text),
          readText: async () => pasted,
        },
      });
    }, PASTED);
    const answers: AnswerRequest[] = [];
    await page.route("**/api/**", async (route) => {
      if (!isAnswerRequest(route.request())) return route.fulfill({ json: {} });
      const body = answerRequest(route.request());
      answers.push(body);
      if (body.decision === "accept")
        return route.fulfill({
          json: answerReply({
            status: "answered",
            openUrl:
              body.id === "redirect" ? phoneSignInUrl : "https://x.invalid/",
          }),
        });
      return route.fulfill({ json: answerReply({ status: "answered" }) });
    });
    // The first load waits for the Vite dependency scan.
    await page.goto(`${ORIGIN}/check`, { timeout: 30_000 });
    expect(await page.evaluate(() => location.hostname)).toBe(PHONE_HOST);
    const redirect = page.locator('[data-request="redirect"]');
    const device = page.locator('[data-request="device"]');
    await redirect.waitFor({ timeout: 30_000 }).catch((error) => {
      throw new Error(`${error.message}\n${errors.join("\n")}`);
    });
    await expect(redirect.getByText("Waiting for you")).toBeVisible();
    await expect(redirect.getByText("Time left")).toBeVisible();
    await expect(redirect.getByText(/^[89] min$/)).toBeVisible();
    await expect(
      redirect.getByText("the chat's server, port 40645"),
    ).toBeVisible();

    const fits = async () =>
      page.evaluate(() => {
        const overflow =
          document.documentElement.scrollWidth -
          document.documentElement.clientWidth;
        const outside = [...document.querySelectorAll("button")].filter(
          (button) => {
            const box = button.getBoundingClientRect();
            return box.left < 0 || box.right > window.innerWidth;
          },
        ).length;
        return { overflow, outside };
      });
    expect(await fits()).toEqual({ overflow: 0, outside: 0 });

    // Device code: the code is large and can be copied; nothing opens.
    await expect(device.getByText("WDJB-MJHT", { exact: true })).toBeVisible();
    await expect(
      device.getByText("Enter it on device.sso.eu-central-1.amazonaws.com"),
    ).toBeVisible();
    await device.getByRole("button", { name: "Copy code" }).click();
    await expect(
      device.getByRole("button", { name: "Code copied" }),
    ).toBeVisible();
    expect(await page.evaluate(() => window.__copied)).toEqual(["WDJB-MJHT"]);
    expect(await page.evaluate(() => window.__opened)).toEqual([]);
    await page
      .screenshot({
        path: testInfo.outputPath("sign-in-card-phone-approval.png"),
        fullPage: true,
      })
      .catch(() => {});

    // Paste mode: the approval opens the provider page in the system browser.
    await redirect.getByRole("button", { name: "Open in browser" }).click();
    await expect
      .poll(() => page.evaluate(() => window.__opened))
      .toEqual([phoneSignInUrl]);
    expect(answers).toEqual([
      { decision: "accept", paste: true, id: "redirect" },
    ]);

    await page.evaluate(() => window.__waiting());
    await expect(redirect.getByText("Waiting for the browser")).toBeVisible();
    await expect(
      redirect.getByText("Copy its address and paste it here.", {
        exact: false,
      }),
    ).toBeVisible();
    const send = redirect.getByRole("button", { name: "Send to command" });
    await expect(send).toBeDisabled();
    await redirect.getByRole("button", { name: "Paste", exact: true }).click();
    await expect(
      redirect.getByLabel("Address of the page that did not load"),
    ).toHaveValue(PASTED);
    expect(await fits()).toEqual({ overflow: 0, outside: 0 });
    await page
      .screenshot({
        path: testInfo.outputPath("sign-in-card-phone-waiting.png"),
        fullPage: true,
      })
      .catch(() => {});
    await send.click();
    await expect.poll(() => answers.length).toBe(2);
    expect(answers[1]).toEqual({
      decision: "answer",
      result: PASTED,
      id: "redirect",
    });
    await expect
      .poll(() => page.evaluate(() => window.__notes))
      .toContain(
        "Studio sent the sign-in result to the command. The chat shows how it ended.",
      );
    // The field is cleared; a second click cannot send the code again.
    await expect(send).toBeDisabled();
    expect(answers).toHaveLength(2);
    expect(await page.evaluate(() => window.__opened)).toEqual([
      phoneSignInUrl,
    ]);
    // The pasted address is not kept in browser storage.
    const stored = await page.evaluate(() =>
      JSON.stringify({ ...localStorage, ...sessionStorage }),
    );
    expect(stored).not.toContain("code=c1");
  } finally {
    await server.close();
  }
});
