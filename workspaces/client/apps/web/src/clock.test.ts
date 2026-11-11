import { autorun } from "mobx";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { now, reached } from "./clock.ts";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

it("ticks only while observed", () => {
  expect(vi.getTimerCount()).toBe(0);
  const seen: number[] = [];
  const stop = autorun(() => seen.push(now()));
  expect(vi.getTimerCount()).toBe(1);
  vi.advanceTimersByTime(3000);
  expect(seen).toHaveLength(4);
  stop();
  expect(vi.getTimerCount()).toBe(0);
});

it("notifies exactly at a deadline, independent of the tick phase", () => {
  const stopTicks = autorun(() => now());
  vi.advanceTimersByTime(100);
  const at = Date.now() + 1000;
  const seen: boolean[] = [];
  const stop = autorun(() => seen.push(reached(at)));
  vi.advanceTimersByTime(999);
  expect(seen).toEqual([false]);
  vi.advanceTimersByTime(1);
  expect(seen).toEqual([false, true]);
  stop();
  stopTicks();
  expect(vi.getTimerCount()).toBe(0);
});
