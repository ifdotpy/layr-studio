import { createAtom, computed } from "mobx";
import { createTokenRateSnapshotMirror } from "../generated/observableResources.ts";
import { observeInterest } from "../store/interest.ts";
import type { components } from "../generated/api.ts";

// This cache receives volatile SSE events. It never enters the sync database.
export type TokenRate = import("../deepReadonly.ts").DeepReadonly<
  components["schemas"]["TokenRateRecord"]
>;
type TokenRateEvent = components["schemas"]["ResourceTokenRatesEvent"];
type WorkspaceRates =
  import("../generated/observableResources.ts").TokenRateSnapshot;

export function tweenTokenRate(from: number, to: number, elapsed: number) {
  const progress = Math.min(1, Math.max(0, elapsed / 450));
  return from + (to - from) * (1 - (1 - progress) ** 3);
}

const standardRate = new Intl.NumberFormat(undefined, {
  maximumFractionDigits: 0,
  useGrouping: false,
});

const compactRate = new Intl.NumberFormat(undefined, {
  // Stryker disable next-line StringLiteral: This mutation throws RangeError at module load; Vitest collects zero tests and reports Survived (static, testsCompleted 0), so no test in this file can report it killed. This is a tool limitation, not an equivalent mutant.
  notation: "compact",
  maximumFractionDigits: 0,
  useGrouping: false,
});

export function formatTokenRate(value: number) {
  if (value > 0 && value < 1) return "<1";
  return (value >= 10000 ? compactRate : standardRate).format(value);
}
export function createTokenRateCache() {
  let watchStream:
    | ((listener: (event: TokenRateEvent) => void) => () => void)
    | undefined;

  function configureTokenRateStream(
    watch: (listener: (event: TokenRateEvent) => void) => () => void,
  ) {
    watchStream = watch;
  }

  const mirror = createTokenRateSnapshotMirror();
  const index = computed(
    () => new Map(mirror.current.records.map((row) => [row.agentId, row])),
  );
  const interest = observeInterest(
    () => watchStream?.(() => {}) ?? (() => {}),
    () => {},
  );
  const atom = createAtom(
    "token-rate-interest",
    interest.observed,
    interest.unobserved,
  );
  function observeTokenRates() {
    atom.reportObserved();
  }
  let baselineEpoch: string | undefined;
  let baselineWorkspaceId: string | undefined;

  function receiveResourceTokenRates(event: TokenRateEvent) {
    if (
      !event.snapshot &&
      (baselineEpoch !== event.epoch ||
        baselineWorkspaceId !== event.workspaceId)
    )
      return false;
    if (event.snapshot) {
      replaceWorkspaceTokenRates(event);
      baselineEpoch = event.epoch;
      baselineWorkspaceId = event.workspaceId;
      return true;
    }
    const next = new Map(index.get());
    for (const id of event.removed) next.delete(id);
    for (const record of event.records) next.set(record.agentId, record);
    mirror.apply({ records: [...next.values()] });
    return true;
  }

  // Materialize the merged baseline only when a late local or peer subscriber asks.
  function getWorkspaceTokenRates(): components["schemas"]["TokenRateSnapshot"] {
    return { records: mirror.current.records.map((record) => ({ ...record })) };
  }

  function receiveWorkspaceTokenRates(value: WorkspaceRates) {
    return replaceWorkspaceTokenRates(value);
  }

  function replaceWorkspaceTokenRates(value: WorkspaceRates) {
    mirror.apply(value);
    return true;
  }

  function clearWorkspaceTokenRates() {
    baselineEpoch = undefined;
    baselineWorkspaceId = undefined;
    receiveWorkspaceTokenRates({ records: [] });
  }

  function getTokenRate(id: string): TokenRate | null {
    observeTokenRates();
    return index.get().get(id) ?? null;
  }

  function getWorkspaceTeamTokenRate(
    teamId: string,
    agentId: string,
  ): TokenRate | null {
    observeTokenRates();
    const record = index.get().get(agentId);
    return record?.rootId === teamId ? record : null;
  }
  return {
    configureTokenRateStream,
    dispose() {
      interest.dispose();
    },
    observeTokenRates,
    receiveResourceTokenRates,
    receiveWorkspaceTokenRates,
    clearWorkspaceTokenRates,
    getWorkspaceTokenRates,
    getTokenRate,
    getWorkspaceTeamTokenRate,
  };
}
export const {
  dispose,
  configureTokenRateStream,
  observeTokenRates,
  receiveResourceTokenRates,
  receiveWorkspaceTokenRates,
  clearWorkspaceTokenRates,
  getWorkspaceTokenRates,
  getTokenRate,
  getWorkspaceTeamTokenRate,
} = createTokenRateCache();
