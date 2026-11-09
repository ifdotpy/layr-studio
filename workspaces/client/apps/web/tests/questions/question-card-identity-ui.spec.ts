import { createBuiltQuestionServer } from "./builtQuestionServer";
import { test, expect } from "../playwright.ts";
import {
  answerReply,
  answerRequest,
  isAnswerRequest,
  type AnswerRequest,
} from "../answer-command";

test("questions and confirmations edit their original mounted card", async ({
  page,
}, testInfo) => {
  const server = await createBuiltQuestionServer(
    "./question-card-identity-ui.fixture.tsx",
  );
  await server.listen();
  const answers: AnswerRequest[] = [];
  let release: (() => void) | undefined;
  try {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.route("**/api/commands", async (route) => {
      if (!isAnswerRequest(route.request())) return route.fallback();
      answers.push(answerRequest(route.request()));
      await new Promise<void>((resolve) => {
        release = resolve;
      });
      await route.fulfill({ json: answerReply({ status: "answered" }) });
    });
    const origin = server.resolvedUrls?.local[0];
    if (!origin) throw new Error("Missing fixture origin");
    await page.goto(origin + "check");
    const question = page.locator('[data-request="question"]');
    await expect(question).toHaveCount(1);
    const original = await question.elementHandle();
    await question.getByRole("button", { name: "Answer", exact: true }).click();
    expect(
      await question.evaluate((node, before) => node === before, original),
    ).toBe(true);
    await expect(
      question.getByRole("form", { name: "Reply to the agent" }),
    ).toBeVisible();
    await expect(page.locator(".question-card")).toHaveCount(0);
    await question.getByRole("button", { name: "Yes", exact: true }).click();
    await question.getByRole("button", { name: "Send answer" }).click();
    await expect.poll(() => answers.length).toBe(1);
    expect(
      await question.evaluate((node, before) => node === before, original),
    ).toBe(true);
    expect(answers[0]).toEqual({
      id: "question",
      answers: { "0": { answers: ["Yes"] } },
    });
    release?.();
    await page.evaluate(() => window.publishAnsweredRequest("question"));
    await expect(
      page.locator('.question-card[data-question-state="answered"]'),
    ).toHaveCount(1);
    await expect(question).toHaveCount(0);

    const approval = page.locator('[data-request="approval"]');
    const originalApproval = await approval.elementHandle();
    await approval
      .getByRole("button", { name: "Approve", exact: true })
      .click();
    await expect.poll(() => answers.length).toBe(2);
    expect(
      await approval.evaluate(
        (node, before) => node === before,
        originalApproval,
      ),
    ).toBe(true);
    await expect(approval).toHaveCount(1);
    await expect(
      approval.getByRole("button", { name: "Approve", exact: true }),
    ).toBeDisabled();
    expect(answers[1]).toEqual({ id: "approval", decision: "accept" });
    release?.();
    await page.evaluate(() => window.publishAnsweredRequest("approval"));
    await expect(approval).toHaveCount(0);
    await expect(
      page.locator('.tool-card[data-message="lead:command-item"]'),
    ).toHaveCount(1);
    await page.screenshot({
      path: testInfo.outputPath("answered-question.png"),
    });
    expect(errors).toEqual([]);
  } finally {
    release?.();
    await server.close();
  }
});
