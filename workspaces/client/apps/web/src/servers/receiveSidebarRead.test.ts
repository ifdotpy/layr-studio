import { expect, it, vi } from "vitest";
import { createStateMirror } from "../generated/observableResources.ts";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import { receiveSidebarRead } from "./receiveSidebarRead.ts";

it("the App receiver writes the clicked CAS target without Agent data even while queued", async () => {
  const mirror = createStateMirror();
  const initial = agentSummaryFixture("chat", {
    markRead: { threadId: "thread", turnId: "turn", read: true, revision: 3 },
  });
  mirror.apply([
    { collection: "agentSummary", id: initial.id, value: initial },
  ]);
  const row = mirror.current.agents.get("chat");
  if (!row?.markRead) throw new Error("Missing fixture target");
  let release = () => {};
  const queue = new Promise<void>((resolve) => {
    release = resolve;
  });
  const post = vi.fn();
  const writing = receiveSidebarRead(
    { action: "mark-unread", id: row.id, target: row.markRead },
    async (id, target) => {
      await queue;
      post({ id, target });
    },
  );
  mirror.apply([
    {
      collection: "agentSummary",
      id: initial.id,
      value: {
        ...initial,
        markRead: {
          threadId: "thread",
          turnId: "new-turn",
          read: true,
          revision: 4,
        },
      },
    },
  ]);
  expect(row.markRead.revision).toBe(4);
  release();
  await writing;
  expect(post).toHaveBeenCalledExactlyOnceWith({
    id: "chat",
    target: { threadId: "thread", turnId: "turn", read: true, revision: 3 },
  });
});
