import { expect, it, vi } from "vitest";
import { createSidebarMirror } from "../generated/observableResources.ts";
import { sidebarRowFixture } from "./sidebarRowFixture.ts";
import { receiveSidebarRead } from "./receiveSidebarRead.ts";

it("the App receiver writes the clicked CAS target without Agent data even while queued", async () => {
  const mirror = createSidebarMirror();
  const initial = sidebarRowFixture("chat", {
    markRead: { threadId: "thread", turnId: "turn", read: true, revision: 3 },
  });
  mirror.apply({ rows: [initial] });
  const row = mirror.current.rows[0];
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
  mirror.apply({
    rows: [
      {
        ...initial,
        markRead: {
          threadId: "thread",
          turnId: "new-turn",
          read: true,
          revision: 4,
        },
      },
    ],
  });
  expect(row.markRead.revision).toBe(4);
  release();
  await writing;
  expect(post).toHaveBeenCalledExactlyOnceWith({
    id: "chat",
    target: { threadId: "thread", turnId: "turn", read: true, revision: 3 },
  });
});
