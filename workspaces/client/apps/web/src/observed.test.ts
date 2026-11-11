import { autorun, observable } from "mobx";
import { expect, it, vi } from "vitest";
import { whileObserved } from "./observed.ts";

it("defers start until after the observing derivation", async () => {
  const state = observable({ value: 0 });
  const release = vi.fn();
  const start = vi.fn(() => release);
  const dispose = whileObserved(state, "value", start);
  const stop = autorun(() => state.value);
  expect(start).not.toHaveBeenCalled();
  await Promise.resolve();
  expect(start).toHaveBeenCalledOnce();
  stop();
  dispose();
  await Promise.resolve();
  expect(release).toHaveBeenCalledOnce();
});

it("never starts an observation that ends within the same tick", async () => {
  const state = observable({ value: 0 });
  const start = vi.fn(() => vi.fn());
  const dispose = whileObserved(state, "value", start);
  autorun(() => state.value)();
  await Promise.resolve();
  expect(start).not.toHaveBeenCalled();
  dispose();
});

it("releases once after the last observer and reuses a same-tick lifetime", async () => {
  const state = observable({ value: 0 });
  const release = vi.fn();
  const start = vi.fn(() => release);
  const dispose = whileObserved(state, "value", start);
  const first = autorun(() => state.value);
  const second = autorun(() => state.value);
  await Promise.resolve();
  first();
  await Promise.resolve();
  expect(release).not.toHaveBeenCalled();
  second();
  const third = autorun(() => state.value);
  await Promise.resolve();
  expect(start).toHaveBeenCalledOnce();
  expect(release).not.toHaveBeenCalled();
  third();
  expect(release).not.toHaveBeenCalled();
  await Promise.resolve();
  expect(release).toHaveBeenCalledOnce();
  dispose();
  await Promise.resolve();
  expect(release).toHaveBeenCalledOnce();
});

it("abandons an unstarted lifetime once after observation ends", async () => {
  const state = observable({ value: 0 });
  const start = vi.fn(() => vi.fn());
  const abandon = vi.fn();
  const dispose = whileObserved(state, "value", start, abandon);
  autorun(() => state.value)();
  expect(abandon).not.toHaveBeenCalled();
  await Promise.resolve();
  expect(start).not.toHaveBeenCalled();
  expect(abandon).toHaveBeenCalledOnce();
  dispose();
  await Promise.resolve();
  expect(abandon).toHaveBeenCalledOnce();
});
