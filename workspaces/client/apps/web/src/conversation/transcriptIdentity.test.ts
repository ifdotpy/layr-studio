import { expect, it } from "vitest";
import { createTranscriptMirror } from "../generated/observableResources.ts";
import { createMessageViews } from "./messageView.ts";

it("generated transcript references preserve native order, nested changes, omission and removal", () => {
  const initial = {
    items: [
      {
        id: "user",
        role: "user",
        text: "saved",
        inputs: [
          { id: "input", kind: "user", text: "saved", truncated: false },
        ],
      },
      { id: "answer", role: "assistant", text: "first", streaming: true },
    ],
    truncated: false,
  };
  const mirror = createTranscriptMirror(initial);
  const views = createMessageViews("agent");
  const before = views(mirror.current.items);
  mirror.apply(structuredClone(initial));
  expect(views(mirror.current.items)[0]).toBe(before[0]);
  const answer = mirror.current.items[1]!;
  mirror.apply({
    items: [
      { id: "answer", role: "assistant", text: "finished" },
      {
        id: "user",
        role: "user",
        text: "saved",
        inputs: [
          { id: "input", kind: "user", text: "changed", truncated: false },
        ],
      },
    ],
    truncated: false,
  });
  const next = views(mirror.current.items);
  expect(next.map((item) => item.id)).toEqual(["answer", "agent:input"]);
  expect(next[0]).toBe(before[1]);
  expect(next[1]).toBe(before[0]);
  expect(next[1]?.text).toBe("changed");
  expect(answer.streaming).toBeUndefined();
  expect(answer.text).toBe("finished");
  mirror.apply({ items: [], truncated: false });
  expect(views(mirror.current.items)).toEqual([]);
});
