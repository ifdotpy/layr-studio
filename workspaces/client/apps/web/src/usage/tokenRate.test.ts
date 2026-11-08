import { autorun, reaction, toJS } from "mobx";
import { expect } from "vitest";
import assert from "node:assert/strict";
import { it as test, vi } from "vitest";
import {
  createTokenRateCache,
  formatTokenRate,
  tweenTokenRate,
} from "./tokenRate.ts";

test("tween and display formatting honor user-visible boundaries", () => {
  assert.equal(tweenTokenRate(1, 9, -1), 1);
  assert.equal(tweenTokenRate(1, 9, 451), 9);
  assert.ok(tweenTokenRate(10, 20, 225) > 10);
  assert.ok(tweenTokenRate(10, 20, 225) < 20);
  assert.equal(formatTokenRate(0), "0");
  assert.equal(formatTokenRate(0.99), "<1");
  assert.equal(formatTokenRate(1), "1");
  assert.equal(formatTokenRate(1.4), "1");
  assert.equal(formatTokenRate(1234), "1234");
  assert.equal(formatTokenRate(9999), "9999");
  assert.equal(
    formatTokenRate(10000),
    new Intl.NumberFormat(undefined, {
      notation: "compact",
      maximumFractionDigits: 0,
      useGrouping: false,
    }).format(10000),
  );
  const runtimeLocaleCompact = new Intl.NumberFormat(undefined, {
    notation: "compact",
    maximumFractionDigits: 0,
    useGrouping: false,
  }).format(1e16);
  assert.equal(formatTokenRate(1e16), runtimeLocaleCompact);
});

test("team and footer observations share stabilized stream interest, replay and release", async () => {
  vi.useFakeTimers();
  const cache = createTokenRateCache();
  const release = vi.fn();
  const open = vi.fn(() => release);
  cache.configureTokenRateStream(open);
  const read = vi.fn();
  try {
    cache.getTokenRate("worker");
    await vi.advanceTimersByTimeAsync(100);
    expect(open).not.toHaveBeenCalled();
    const stop = autorun(() => {
      cache.getTokenRate("worker");
    });
    stop();
    await vi.advanceTimersByTimeAsync(100);
    expect(open).not.toHaveBeenCalled();
    const stopFooter = reaction(
      () => toJS(cache.getTokenRate("worker")),
      read,
      { fireImmediately: true },
    );
    const stopTeam = autorun(() => {
      cache.getWorkspaceTeamTokenRate("team", "worker");
    });
    await vi.advanceTimersByTimeAsync(100);
    expect(open).toHaveBeenCalledOnce();
    const record = {
      agentId: "worker",
      rootId: "team",
      turnId: "turn",
      active: true,
      estimated: false,
      rate: 42,
      outputTokens: 84,
    };
    cache.receiveWorkspaceTokenRates({ records: [record] });
    expect(read).toHaveBeenLastCalledWith(record, null, expect.anything());
    stopFooter();
    expect(release).not.toHaveBeenCalled();
    stopTeam();
    expect(release).toHaveBeenCalledOnce();
    const stopLate = autorun(() => {
      expect(cache.getTokenRate("worker")?.rate).toBe(42);
    });
    await vi.advanceTimersByTimeAsync(100);
    expect(open).toHaveBeenCalledTimes(2);
    stopLate();
  } finally {
    cache.dispose();
    vi.useRealTimers();
  }
});
