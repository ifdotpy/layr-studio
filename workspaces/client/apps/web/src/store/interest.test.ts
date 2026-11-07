import { autorun, createAtom } from "mobx";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { observeInterest } from "./interest.ts";

describe("observable resource interest", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("opens only after the atom remains observed for 100 ms", () => {
    const release = vi.fn();
    const acquire = vi.fn(() => release);
    const inactive = vi.fn();
    const interest = observeInterest(acquire, inactive);
    const atom = createAtom("slot", interest.observed, interest.unobserved);
    // An action or other untracked read must open nothing.
    atom.reportObserved();
    vi.advanceTimersByTime(100);
    expect(acquire).not.toHaveBeenCalled();
    const stop = autorun(() => atom.reportObserved());
    vi.advanceTimersByTime(99);
    expect(acquire).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(acquire).toHaveBeenCalledTimes(1);
    stop();
    expect(inactive).toHaveBeenCalledTimes(1);
    // The existing reference-counted store, not this helper, owns release grace.
    expect(release).toHaveBeenCalledTimes(1);
    interest.dispose();
  });

  it("cancels short observations and disposal cancels a pending acquisition", () => {
    const acquire = vi.fn(() => vi.fn());
    const interest = observeInterest(acquire, vi.fn());
    const atom = createAtom("slot", interest.observed, interest.unobserved);
    const stop = autorun(() => atom.reportObserved());
    vi.advanceTimersByTime(50);
    stop();
    vi.advanceTimersByTime(100);
    expect(acquire).not.toHaveBeenCalled();
    const again = autorun(() => atom.reportObserved());
    interest.dispose();
    vi.advanceTimersByTime(100);
    expect(acquire).not.toHaveBeenCalled();
    again();
  });
});
