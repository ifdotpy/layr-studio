import { BackgroundTaskIndicator } from "../../generated/api";
import { describe, expect, it } from "vitest";
import type { Agent } from "../../types";
import {
  backgroundTaskState,
  responseOwnerIdentityKey,
  responseStopIdentity,
  responseStopDisabled,
} from "./BackgroundTasks";

describe("response stop identity", () => {
  it("changes when the owner moves from idle to active or starts a new turn", () => {
    const idle = {
      id: "lead",
      accountKey: "default",
      epoch: 4,
      inFlight: false,
      threadId: "thread-current",
      turnId: null,
    } satisfies Agent;
    const active = { ...idle, inFlight: true, turnId: "turn-1" };
    const nextTurn = { ...active, turnId: "turn-2" };
    expect(responseOwnerIdentityKey(idle)).not.toBe(
      responseOwnerIdentityKey(active),
    );
    expect(responseOwnerIdentityKey(active)).not.toBe(
      responseOwnerIdentityKey(nextTurn),
    );
  });

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
    expect(responseStopDisabled(target, false)).toBe(false);
    expect(responseStopDisabled(target, true)).toBe(true);
    expect(responseStopDisabled(null, false)).toBe(true);
    expect(responseStopDisabled(undefined, false)).toBe(true);
  });
});

describe("dispatch strip states", () => {
  it("uses the indicator published by the server", () => {
    const display = {
      kind: "monitor",
      kindLabel: "monitor",
      title: "Command",
      active: true,
      indicator: "answer",
      label: "approval",
      inBackgroundList: true,
      actions: ["approve", "readOutput"],
    } as const;
    expect(backgroundTaskState({ display })).toBe("answer");
    expect(
      backgroundTaskState({
        display: { ...display, indicator: BackgroundTaskIndicator.working },
      }),
    ).toBe("working");
    expect(
      backgroundTaskState({
        display: { ...display, indicator: BackgroundTaskIndicator.error },
      }),
    ).toBe("error");
  });
});
