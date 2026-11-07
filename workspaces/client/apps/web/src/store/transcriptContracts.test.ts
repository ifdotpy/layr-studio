import { expect, it } from "vitest";
import { createTranscriptMirror } from "../generated/observableResources.ts";

it("reconciles incoming transcript order and retains item and input identities", () => {
  const first = {
    id: "b",
    role: "assistant",
    text: "B",
    inputs: [
      { id: "input", kind: "text", text: "first", truncated: false },
      { kind: "text", text: "legacy", truncated: false },
    ],
  };
  const mirror = createTranscriptMirror({
    items: [first, { id: "a", role: "assistant", text: "A" }],
  });
  const b = mirror.current.items[0]!;
  const a = mirror.current.items[1]!;
  const input = b.inputs![0];
  const legacy = b.inputs![1];
  mirror.apply({
    items: [
      { id: "a", role: "assistant", text: "A" },
      {
        ...first,
        inputs: [
          { id: "input", kind: "text", text: "updated", truncated: false },
          { kind: "text", text: "legacy updated", truncated: false },
        ],
      },
    ],
  });
  expect(mirror.current.items.map((item) => item.id)).toEqual(["a", "b"]);
  expect(mirror.current.items[0]).toBe(a);
  expect(mirror.current.items[1]).toBe(b);
  expect(b.inputs![0]).toBe(input);
  expect(b.inputs![1]).toBe(legacy);
  expect(input!.text).toBe("updated");
  expect(legacy!.text).toBe("legacy updated");
});
