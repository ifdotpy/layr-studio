import { makeAutoObservable } from "mobx";
import { afterEach, expect, it, vi } from "vitest";
import { DisplayPhase } from "./useDisplayPhase.ts";

afterEach(() => vi.useRealTimers());

it("keeps brief phase transitions in the work log and cancels superseded phases", () => {
  vi.useFakeTimers();
  const source = makeAutoObservable({
    scope: "chat",
    phase: "thinking",
    set(phase: string) {
      this.phase = phase;
    },
  });
  const phase = new DisplayPhase(() => [source.scope, source.phase]);
  const dispose = phase.start();
  source.set("tool");
  vi.advanceTimersByTime(200);
  expect(phase.current).toBe("thinking");
  source.set("reasoning");
  vi.advanceTimersByTime(249);
  expect(phase.current).toBe("thinking");
  vi.advanceTimersByTime(1);
  expect(phase.current).toBe("reasoning");
  dispose();
});

it("changes scope immediately and disposes a pending transition", () => {
  vi.useFakeTimers();
  const source = makeAutoObservable({
    scope: "one",
    phase: "thinking",
    set(scope: string, phase: string) {
      Object.assign(this, { scope, phase });
    },
  });
  const phase = new DisplayPhase(() => [source.scope, source.phase]);
  const dispose = phase.start();
  source.set("one", "tool");
  source.set("two", "error");
  expect(phase.current).toBe("error");
  vi.advanceTimersByTime(250);
  expect(phase.current).toBe("error");
  source.set("two", "thinking");
  dispose();
  vi.advanceTimersByTime(250);
  expect(phase.current).toBe("error");
  expect(vi.getTimerCount()).toBe(0);
});
