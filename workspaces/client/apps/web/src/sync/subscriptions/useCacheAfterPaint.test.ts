import { afterEach, expect, it, vi } from "vitest";

const fixture = vi.hoisted(() => ({
  effect: undefined as (() => void | (() => void)) | undefined,
  save: vi.fn(),
}));
vi.mock("react", () => ({
  useEffect: (effect: () => void | (() => void)) => {
    fixture.effect = effect;
  },
}));
vi.mock("../../servers/ServerServicesContext", () => ({
  useServerServices: () => ({ sync: { cacheSubscription: fixture.save } }),
}));
import { useCacheAfterPaint } from "./useCacheAfterPaint";

afterEach(() => {
  vi.unstubAllGlobals();
  fixture.save.mockReset();
});

it("schedules caching only after the committed view has painted", () => {
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    frames.push(callback);
    return frames.length;
  });
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  const ref = { kind: "state" } as const;
  useCacheAfterPaint(ref, { visible: true });
  expect(fixture.save).not.toHaveBeenCalled();
  fixture.effect?.();
  expect(fixture.save).not.toHaveBeenCalled();
  frames.shift()?.(0);
  expect(fixture.save).not.toHaveBeenCalled();
  frames.shift()?.(16);
  expect(fixture.save).toHaveBeenCalledWith(ref);
});
