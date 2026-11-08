// Answers of the one command path: `POST /api/commands` with `kind: "answer"`. The approval id
// is the command target, and the answer fields are the body.
import type { Request } from "@playwright/test";
import type { components } from "../src/generated/api.ts";

export type AnswerResult = components["schemas"]["AnswerResult"];
export type AnswerRequest = { id: string } & Record<string, unknown>;

/** True for an answer command. Other commands pass through to their own handlers. */
export function isAnswerRequest(request: Request): boolean {
  return (
    request.method() === "POST" &&
    new URL(request.url()).pathname === "/api/commands" &&
    (request.postDataJSON() as { kind?: unknown } | null)?.kind === "answer"
  );
}

/** The answer fields of an answer command, with the approval id as `id`. */
export function answerRequest(request: Request): AnswerRequest {
  const command = request.postDataJSON() as {
    target: string;
    body: Record<string, unknown>;
  };
  return { ...command.body, id: command.target };
}

/** The response of an applied answer command. */
export function answerReply(result: AnswerResult) {
  return {
    status: "applied",
    ref_kind: "approval",
    ref_id: "approval",
    rev: 0,
    seq: 0,
    rows: [],
    kind: "answer",
    result,
  } as const;
}
