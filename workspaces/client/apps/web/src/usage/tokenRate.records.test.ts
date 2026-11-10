import { reaction, toJS, compareStructural } from "mobx";
import type { components } from "../generated/api.ts";
import { expect, it, vi } from "vitest";
import { SYNC_PROTOCOL_VERSION } from "../generated/api.ts";
import { createTokenRateCache, type TokenRate } from "./tokenRate.ts";

it("one token record drives footer and team without an agent projection, moves roots and clears removals", () => {
  const cache = createTokenRateCache();
  const record = {
    agentId: "worker",
    rootId: "first",
    turnId: "turn",
    active: true,
    estimated: false,
    rate: 42,
    outputTokens: 84,
  } satisfies TokenRate;
  const footer: (TokenRate | null)[] = [],
    first: (TokenRate | null)[] = [],
    second: (TokenRate | null)[] = [];
  const closeFooter = reaction(
    () => toJS(cache.getTokenRate("worker")),
    (value) => footer.push(value),
    { fireImmediately: true },
  );
  const closeFirst = reaction(
    () => toJS(cache.getWorkspaceTeamTokenRate("first", "worker")),
    (value) => first.push(value),
    { fireImmediately: true },
  );
  const closeSecond = reaction(
    () => toJS(cache.getWorkspaceTeamTokenRate("second", "worker")),
    (value) => second.push(value),
    { fireImmediately: true },
  );
  expect(cache.receiveWorkspaceTokenRates({ records: [record] })).toBe(true);
  expect(footer.at(-1)).toEqual(record);
  expect(first.at(-1)).toEqual(record);
  expect(second.at(-1)).toBeNull();
  const moved = { ...record, rootId: "second" };
  expect(cache.receiveWorkspaceTokenRates({ records: [moved] })).toBe(true);
  expect(footer.at(-1)).toEqual(moved);
  expect(first.at(-1)).toBeNull();
  expect(second.at(-1)).toEqual(moved);
  expect(cache.receiveWorkspaceTokenRates({ records: [] })).toBe(true);
  expect(footer.at(-1)).toBeNull();
  expect(second.at(-1)).toBeNull();
  closeFooter();
  closeFirst();
  closeSecond();
  cache.dispose();
});

it("applies token-rate deltas atomically, preserves quiet records, and replaces on reconnect", () => {
  const cache = createTokenRateCache();
  const record: TokenRate = {
    agentId: "active",
    rootId: "first",
    turnId: "turn",
    active: true,
    estimated: false,
    rate: 1,
    outputTokens: 10,
  };
  const quiet = { ...record, agentId: "quiet", rate: 0 };
  const gone = { ...record, agentId: "gone" };
  const other = { ...record, agentId: "other" };
  const frame = (
    snapshot: boolean,
    records: TokenRate[],
    removed: string[] = [],
  ) =>
    ({
      protocol: SYNC_PROTOCOL_VERSION,
      workspaceId: "workspace",
      epoch: "epoch",
      revision: 1,
      snapshot,
      records,
      removed,
    }) satisfies components["schemas"]["ResourceTokenRatesEvent"];
  expect(
    cache.receiveResourceTokenRates(frame(true, [record, quiet, gone, other])),
  ).toBe(true);
  const observations: unknown[] = [];
  const stop = reaction(
    () =>
      toJS([
        cache.getTokenRate("active"),
        cache.getTokenRate("quiet"),
        cache.getTokenRate("gone"),
        cache.getWorkspaceTeamTokenRate("first", "active"),
        cache.getWorkspaceTeamTokenRate("second", "active"),
        cache.getTokenRate("other"),
      ]),
    (values) => observations.push(values.slice(1)),
    { equals: compareStructural },
  );
  observations.length = 0;
  const moved = { ...record, rootId: "second", rate: 5 };
  const updatedOther = { ...other, rate: 8 };
  expect(
    cache.receiveResourceTokenRates(
      frame(false, [moved, updatedOther], ["gone"]),
    ),
  ).toBe(true);
  expect(observations).toEqual([[quiet, null, null, moved, updatedOther]]);
  expect(cache.getTokenRate("quiet")).toEqual(quiet);
  expect(cache.receiveResourceTokenRates(frame(true, [quiet]))).toBe(true);
  expect(cache.getTokenRate("active")).toBeNull();
  stop();
  cache.dispose();
});

it("requires a token baseline for each epoch before applying deltas", () => {
  const cache = createTokenRateCache();
  const record = {
    agentId: "quiet",
    rootId: "root",
    turnId: "turn",
    active: false,
    estimated: false,
    rate: 0,
    outputTokens: 10,
  };
  const frame = {
    protocol: SYNC_PROTOCOL_VERSION,
    workspaceId: "workspace",
    epoch: "one",
    revision: 7,
    snapshot: false,
    records: [record],
    removed: [],
  };
  expect(cache.receiveResourceTokenRates(frame)).toBe(false);
  expect(cache.getTokenRate("quiet")).toBeNull();
  expect(cache.receiveResourceTokenRates({ ...frame, snapshot: true })).toBe(
    true,
  );
  expect(cache.receiveResourceTokenRates({ ...frame, epoch: "two" })).toBe(
    false,
  );
  expect(cache.getTokenRate("quiet")).toEqual(record);
  expect(
    cache.receiveResourceTokenRates({ ...frame, epoch: "two", snapshot: true }),
  ).toBe(true);
  expect(
    cache.receiveResourceTokenRates({
      ...frame,
      epoch: "two",
      records: [],
      removed: ["quiet"],
    }),
  ).toBe(true);
  cache.clearWorkspaceTokenRates();
  expect(cache.receiveResourceTokenRates({ ...frame, epoch: "two" })).toBe(
    false,
  );
});

it("equal replay keeps generated record identities and reactions quiet", () => {
  const cache = createTokenRateCache();
  const record = {
    agentId: "agent",
    rootId: "root",
    turnId: "turn",
    active: true,
    estimated: false,
    rate: 3,
    outputTokens: 6,
  };
  cache.receiveWorkspaceTokenRates({ records: [record] });
  const model = cache.getTokenRate("agent");
  const changed = vi.fn();
  const stop = reaction(() => toJS(cache.getTokenRate("agent")), changed);
  cache.receiveWorkspaceTokenRates({ records: [{ ...record }] });
  expect(cache.getTokenRate("agent")).toBe(model);
  expect(changed).not.toHaveBeenCalled();
  cache.receiveWorkspaceTokenRates({ records: [{ ...record, rate: 4 }] });
  expect(changed).toHaveBeenCalledOnce();
  expect(cache.getTokenRate("agent")).toBe(model);
  stop();
  cache.dispose();
});
