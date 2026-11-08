import assert from "node:assert/strict";
import { builtTerminalFixture } from "./builtTerminalFixture.ts";
// Exercise the Codex 0.159 write_stdin_approval request through the real card and answer API.
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "../playwright.ts";
import {
  answerReply,
  answerRequest,
  isAnswerRequest,
  type AnswerRequest,
} from "../answer-command";

test("Terminal Input Approval Ui", async ({
  browser: _testBrowser,
  context: _testContext,
  page: testPage,
}) => {
  const root = fileURLToPath(new URL("../../../../../../", import.meta.url));
  const temporary = await mkdtemp(join(tmpdir(), "studio-stdin-approval-"));

  const server = await builtTerminalFixture(
    join(root, "workspaces/client/apps/web"),
    temporary,
    "./stdin-approval-fixture.tsx",
  );
  await server.listen();
  try {
    const page = testPage;
    page.setDefaultTimeout(5000);
    const answers: AnswerRequest[] = [];
    await page.route("**/api/**", async (route) => {
      if (isAnswerRequest(route.request())) {
        answers.push(answerRequest(route.request()));
        return route.fulfill({ json: answerReply({ status: "answered" }) });
      }
      return route.fulfill({ json: {} });
    });
    await page.route("**/check", (route) =>
      route.fulfill({
        contentType: "text/html",
        body: '<!doctype html><div id="root"></div><script type="module">import "/fixture.js";</script>',
      }),
    );
    await page.goto(server.resolvedUrls!.local[0]! + "check");
    await page.waitForFunction(() =>
      document.querySelector('[data-request="request-159"]'),
    );
    await page
      .getByText("Terminal input approval required", { exact: true })
      .waitFor();
    await page
      .getByText(
        "Send input to an existing terminal to continue the reviewed command.",
        { exact: true },
      )
      .waitFor();
    await page
      .getByRole("button", { name: "Allow input", exact: true })
      .click();
    const deadline = Date.now() + 5000;
    while (!answers.length && Date.now() < deadline)
      await new Promise((resolve) => setTimeout(resolve, 10));
    assert.deepEqual(answers, [{ decision: "accept", id: "request-159" }]);
    console.log(
      "PASS: 0.159 writeStdin request renders its reason and submits one approval reply",
    );
  } finally {
    await server.close();
    await rm(temporary, { recursive: true, force: true });
  }
});
