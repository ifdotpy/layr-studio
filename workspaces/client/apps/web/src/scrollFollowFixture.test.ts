import { expect, test, vi } from "vitest";
import { streamTranscriptFixture } from "../tests/fixtures/streaming-transcript.ts";

test("the scroll fixture publishes only when the witness requests a commit", async () => {
  const publish = vi.fn(async () => "first delta");
  const rendered = vi.fn(async () => {});
  const nextRender = streamTranscriptFixture({ publish, rendered });
  expect(publish).not.toHaveBeenCalled();
  await nextRender();
  expect(publish).toHaveBeenCalledTimes(1);
  expect(rendered).toHaveBeenCalledWith("first delta");
});

test("the scroll fixture waits for the requested delta to render", async () => {
  const admission = Promise.withResolvers<void>();
  const publish = vi.fn(async () => "first delta");
  const entered = Promise.withResolvers<void>();
  const rendered = vi.fn(() => {
    entered.resolve();
    return admission.promise;
  });
  const nextRender = streamTranscriptFixture({ publish, rendered });
  const committed = vi.fn();
  const update = nextRender().then(committed);
  await entered.promise;
  expect(rendered).toHaveBeenCalledWith("first delta");
  expect(committed).not.toHaveBeenCalled();
  admission.resolve();
  await update;
  expect(committed).toHaveBeenCalledTimes(1);
  expect(publish).toHaveBeenCalledTimes(1);
});

test("the scroll fixture propagates a render failure", async () => {
  const error = new Error("The streamed reply did not render");
  const publish = vi.fn(async () => "unrendered delta");
  const nextRender = streamTranscriptFixture({
    publish,
    rendered: async () => {
      throw error;
    },
  });
  await expect(nextRender()).rejects.toBe(error);
  expect(publish).toHaveBeenCalledTimes(1);
});

test("a newer cursor keeps the anchor even at the bottom of the retained page", async () => {
  const { shouldFollowTranscriptTail } =
    await import("./components/useConversationScroll");
  expect(shouldFollowTranscriptTail(0, false)).toBe(false);
  expect(shouldFollowTranscriptTail(0, true)).toBe(true);
  expect(shouldFollowTranscriptTail(100, true)).toBe(false);
});

test("a remembered history anchor resumes without following the tail", async () => {
  const { anchors, followsStoredAnchor } =
    await import("./components/conversation/transcript/historyAnchorStore");
  anchors.set("chat-a", { id: "message", offset: 12 });
  expect(followsStoredAnchor("chat-a")).toBe(false);
  expect(followsStoredAnchor("fresh-chat")).toBe(true);
  anchors.set("chat-a", null);
  expect(followsStoredAnchor("chat-a")).toBe(true);
  anchors.delete("chat-a");
});
