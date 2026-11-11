import type { CommandExecutionRequestApprovalParams } from "../../../../../providers/apps/claude-bridge/codex-generated/protocol.ts";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, rustUiFixture, spawnFixture, test } from "../playwright.ts";

// An approval for a long command keeps the card inside the chat: the command
// wraps or scrolls inside its own box, and Decline and Approve stay visible.
const COMMAND =
  "/bin/zsh -lc 'cd /Users/someone/Projects/some-very-long-project-name/workspaces/client/apps/web && " +
  "pnpm exec playwright test tests/approvals/long-command.spec.ts --reporter=line --workers=1 && " +
  'for f in $(git ls-files "*.ts" | head -200); do node scripts/check-one.ts "$f" --strict || exit 1; done\n' +
  "cat <<EOF > notes.txt\n" +
  "a line of heredoc content that is long enough to wrap ".repeat(40) +
  "\nEOF\n" +
  "x".repeat(600) +
  "'";

test("approval with a long command keeps its card and actions in view", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const root = await mkdtemp(join(tmpdir(), "studio-long-approval-"));
  const fixture = spawnFixture(process.execPath, [rustUiFixture, root], {
    stdio: ["pipe", "pipe", "pipe"],
    env: {
      ...process.env,
      FIXTURE_WORKER_COUNT: "0",
      FIXTURE_SCRIPT: JSON.stringify([
        {
          prompt: "Run a long command",
          steps: [
            {
              request: {
                method: "item/commandExecution/requestApproval",
                params: {
                  command: COMMAND,
                  itemId: "long-command",
                  startedAtMs: Date.now(),
                  reason: "Approve the long command",
                  cwd: "/Users/someone/Projects/some-very-long-project-name/workspaces/client/apps/web",
                } satisfies Omit<
                  CommandExecutionRequestApprovalParams,
                  "threadId" | "turnId"
                >,
              },
            },
          ],
        },
      ]),
    },
  });
  let log = "";
  fixture.stderr?.on("data", (chunk) => (log += chunk));
  const port = await new Promise<number>((resolve, reject) => {
    fixture.stdout?.once("data", (chunk) =>
      resolve(Number(String(chunk).trim())),
    );
    fixture.once("exit", () => reject(new Error(log)));
  });
  const origin = `http://127.0.0.1:${port}`;
  try {
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto(origin);
    await page.locator("[data-chat]", { hasText: "Release lead" }).click();
    // Full access answers approvals by policy; Ask shows the card.
    await page.getByRole("button", { name: "Chat actions" }).click();
    await page.getByRole("menuitem", { name: "Chat settings" }).click();
    await page
      .getByRole("radiogroup", { name: "Full access without approval" })
      .getByText("Ask", { exact: true })
      .click();
    await expect(
      page.getByRole("radio", { name: "Ask", exact: true }),
    ).toBeChecked();
    await page.keyboard.press("Escape");
    await page.locator("#message").fill("Run a long command");
    await page.locator("#send").click();
    await page.getByRole("button", { name: "Approve", exact: true }).waitFor();
    for (const [width, height] of [
      [1280, 800],
      [700, 900],
      [390, 844],
    ] as const) {
      await page.setViewportSize({ width, height });
      const layout = await page.evaluate(() => {
        const box = (selector: string) =>
          document.querySelector(selector)!.getBoundingClientRect();
        const card = box(".request-card");
        const conversation = box("#conversation");
        const command =
          document.querySelector<HTMLElement>(".request-command")!;
        const buttons = [
          ...document.querySelectorAll(".request-actions button"),
        ].map((button) => button.getBoundingClientRect());
        return {
          cardInside:
            card.left >= conversation.left - 1 &&
            card.right <= conversation.right + 1,
          commandInside:
            command.getBoundingClientRect().right <= card.right + 1,
          commandHeight: command.getBoundingClientRect().height,
          buttonsVisible: buttons.every(
            (button) =>
              button.width > 0 &&
              button.top >= 0 &&
              button.bottom <= innerHeight &&
              button.right <= card.right + 1,
          ),
          pageScroll: document.documentElement.scrollWidth > innerWidth + 1,
        };
      });
      await page.screenshot({
        path: test.info().outputPath(`approval-${width}.png`),
      });
      expect(layout, `${width}px`).toEqual({
        cardInside: true,
        commandInside: true,
        commandHeight: expect.any(Number),
        buttonsVisible: true,
        pageScroll: false,
      });
      expect(layout.commandHeight, `${width}px`).toBeLessThanOrEqual(
        height / 2,
      );
    }
  } finally {
    fixture.kill("SIGTERM");
  }
});
