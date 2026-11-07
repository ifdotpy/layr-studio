import { createBuiltQuestionServer } from "./builtQuestionServer";
import { browserSignInUrl } from "./sign-in.fixture.ts";
import type { DesktopBridge } from "../../src/desktop.ts";
import type {} from "./browser-open-approval-ui.fixture.tsx";
// An agent's sign-in link (browser/open) renders as a sign-in
// card. Nothing opens before the approval; after it, the desktop opens only the link that the
// server returned. A refusal from the server, such as a remote window, opens nothing.

import { test, expect } from "../playwright.ts";
import {
  answerReply,
  answerRequest,
  isAnswerRequest,
  type AnswerRequest,
} from "../answer-command";

const REMOTE =
  "Open this chat in the window of the Mac that runs its server to sign in, or use the tool's device-code mode.";

test("Browser Open Approval Ui", async ({ page }) => {
  const server = await createBuiltQuestionServer(
    "browser-open-approval-ui.fixture.tsx",
  );
  await server.listen();
  try {
    page.setDefaultTimeout(5000);
    // The desktop bridge records what it would open; no real browser opens.
    await page.addInitScript(() => {
      window.browserApprovalOpened = [];
      // The bridge fixture omits unused native methods.
      type FixtureBridge = Pick<DesktopBridge, "platform"> & {
        [K in "openExternal" | "openSignIn"]: (
          ...args: Parameters<NonNullable<DesktopBridge[K]>>
        ) => Promise<unknown>;
      };
      const fixtureWindow = window as Omit<Window, "codexDesktop"> & {
        codexDesktop: FixtureBridge;
      };
      fixtureWindow.codexDesktop = {
        platform: "test",
        openExternal: async (
          url: Parameters<NonNullable<DesktopBridge["openExternal"]>>[0],
        ) => {
          return window.browserApprovalOpened.push(["external", url]);
        },
        openSignIn: async (
          url: Parameters<NonNullable<DesktopBridge["openSignIn"]>>[0],
        ) => {
          return window.browserApprovalOpened.push(["signIn", url]);
        },
      };
    });
    const answers: AnswerRequest[] = [];
    await page.route("**/api/**", async (route) => {
      if (!isAnswerRequest(route.request())) return route.fulfill({ json: {} });
      const body = answerRequest(route.request());
      answers.push(body);
      if (body.id === "remote")
        return route.fulfill({ status: 400, json: { error: REMOTE } });
      if (body.decision === "accept")
        return route.fulfill({
          json: answerReply({
            status: "answered",
            openUrl:
              body.id === "approve" ? browserSignInUrl : "https://x.invalid/",
          }),
        });
      return route.fulfill({ json: answerReply({ status: "answered" }) });
    });
    // The first load waits for the Vite dependency scan, which takes longer than the
    // 5 s action limit on a busy machine.
    await page.goto(server.resolvedUrls!.local[0]! + "check", {
      timeout: 30_000,
    });
    const approve = page.locator('[data-request="approve"]');
    await approve.waitFor({ timeout: 30_000 });
    await expect(approve.getByText("Cluster worker")).toBeVisible();
    await expect(approve.getByText("Sign-in request")).toBeVisible();
    await expect(approve.getByText("Browser sign-in")).toBeVisible();
    await expect(
      approve.getByText("accounts.example.com", { exact: true }),
    ).toBeVisible();
    await expect(approve.getByText("kube", { exact: true })).toBeVisible();
    await expect(
      approve.getByText("the chat's server, port 8000"),
    ).toBeVisible();
    // The card offers no direct link: the link opens only through the approval.
    await expect(approve.locator("a[href]")).toHaveCount(0);
    const device = page.locator('[data-request="device"]');
    await expect(device.getByText("No result to forward")).toBeVisible();
    await expect(device.getByText("Device code")).toBeVisible();
    await expect(device.getByText("ABCD", { exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.browserApprovalOpened)).toEqual([]);

    await approve
      .getByRole("button", { name: "Open in browser", exact: true })
      .click();
    await expect
      .poll(() => page.evaluate(() => window.browserApprovalOpened))
      .toEqual([["signIn", browserSignInUrl]]);

    const remote = page.locator('[data-request="remote"]');
    await remote
      .getByRole("button", { name: "Open in browser", exact: true })
      .click();
    await expect
      .poll(() => page.evaluate(() => window.__notes))
      .toContain(REMOTE);
    await expect(remote.getByRole("alert")).toBeVisible();

    await page
      .locator('[data-request="device"]')
      .getByRole("button", { name: "Decline", exact: true })
      .click();
    await expect.poll(() => answers.length).toBe(3);
    expect(answers).toEqual([
      { decision: "accept", id: "approve" },
      { decision: "accept", id: "remote" },
      { decision: "decline", id: "device" },
    ]);
    expect(await page.evaluate(() => window.browserApprovalOpened)).toEqual([
      ["signIn", browserSignInUrl],
    ]);
  } finally {
    await server.close();
  }
});
