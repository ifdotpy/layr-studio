import { describe, expect, it } from "vitest";
import { responseStopIdentity } from "../support/backgroundTaskIdentity.ts";

describe("response stop identity", () => {
  it("sends the exact server-listed connection, thread, and turn", () => {
    const target = {
      agent: "lead",
      accountKey: "default",
      epoch: 4,
      connectionId: "connection-current",
      threadId: "thread-current",
      turnId: "turn-current",
    } satisfies NonNullable<Parameters<typeof responseStopIdentity>[0]>;
    expect(responseStopIdentity(target)).toEqual({ responseTarget: target });
    // A held response can have an exact server target without an in-flight actor.
  });
});

it("captures the response stop proof before its generated model changes", () => {
  const target = {
    agent: "lead",
    accountKey: "default",
    epoch: 4,
    connectionId: "connection",
    threadId: "thread",
    turnId: "turn",
  };
  const captured = responseStopIdentity(target);
  target.turnId = "next-turn";
  expect(captured.responseTarget.turnId).toBe("turn");
});
