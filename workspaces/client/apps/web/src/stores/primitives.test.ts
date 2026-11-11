import { describe, expect, it } from "vitest";
import { DisclosureState } from "./DisclosureState";

describe("DisclosureState", () => {
  it("opens, closes and toggles", () => {
    const state = new DisclosureState();
    state.toggle();
    expect(state.opened).toBe(true);
    state.close();
    expect(state.opened).toBe(false);
    state.set(true);
    expect(state.opened).toBe(true);
  });
});

describe("textField", () => {
  it("reads and writes one string field", async () => {
    const { observable } = await import("mobx");
    const { textField } = await import("./fields");
    const model = observable({ name: "a" });
    const field = textField(model, "name");
    expect(field.value).toBe("a");
    field.onChange({ currentTarget: { value: "b" } } as never);
    expect(model.name).toBe("b");
  });
});

describe("AsyncFeedback", () => {
  it("tracks pending, ignores a second run, and records errors", async () => {
    const { AsyncFeedback } = await import("./AsyncFeedback");
    const { flowResult } = await import("mobx");
    const feedback = new AsyncFeedback();
    let release: (value: number) => void = () => {};
    const accepted: number[] = [];
    const first = flowResult(
      feedback.run(
        () => new Promise<number>((resolve) => (release = resolve)),
        (v) => accepted.push(v),
      ),
    );
    expect(feedback.pending).toBe(true);
    let secondCalls = 0;
    await flowResult(feedback.run(async () => secondCalls++));
    expect(secondCalls).toBe(0);
    release(7);
    await first;
    expect(accepted).toEqual([7]);
    expect(feedback.pending).toBe(false);
    await flowResult(feedback.run(() => Promise.reject(new Error("boom"))));
    expect(feedback.error).not.toBe("");
  });
});
