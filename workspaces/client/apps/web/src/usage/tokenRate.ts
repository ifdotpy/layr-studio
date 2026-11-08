import { createAtom, computed } from "mobx";
import { createTokenRateSnapshotMirror } from "../generated/observableResources.ts";
import { observeInterest } from "../store/interest.ts";
import type { components } from "../generated/api.ts";

import {
  TOKEN_RATE_MAX_ENTRIES,
  TOKEN_RATE_IDENTIFIER_MAX_CODEPOINTS,
} from "../generated/api.ts";

// This cache receives volatile SSE events. It never enters the sync database.
export type TokenRate = import("../deepReadonly.ts").DeepReadonly<
  components["schemas"]["TokenRateRecord"]
>;
type TokenRateEvent = components["schemas"]["ResourceTokenRatesEvent"];
type WorkspaceRates =
  import("../generated/observableResources.ts").TokenRateSnapshot;

const recordKeys = {
  agentId: true,
  rootId: true,
  turnId: true,
  active: true,
  estimated: true,
  rate: true,
  outputTokens: true,
} satisfies Record<keyof TokenRate, true>;

function validIdentifier(value: unknown): value is string {
  if (typeof value !== "string" || !value.length) return false;
  let codepoints = 0;
  for (const _character of value) {
    if (++codepoints > TOKEN_RATE_IDENTIFIER_MAX_CODEPOINTS) return false;
  }
  return true;
}

// One validation at the untrusted snapshot boundary, using generated types and limits.
function isTokenRateSnapshot(
  value: unknown,
  agents = new Set<string>(),
): value is WorkspaceRates {
  if (
    !value ||
    typeof value !== "object" ||
    !("records" in value) ||
    !Array.isArray(value.records) ||
    value.records.length > TOKEN_RATE_MAX_ENTRIES
  )
    return false;
  return value.records.every((record: unknown): record is TokenRate => {
    if (!record || typeof record !== "object" || Array.isArray(record))
      return false;
    if (
      !("agentId" in record) ||
      !validIdentifier(record.agentId) ||
      agents.has(record.agentId)
    )
      return false;
    agents.add(record.agentId);
    return (
      "rootId" in record &&
      validIdentifier(record.rootId) &&
      "turnId" in record &&
      validIdentifier(record.turnId) &&
      "active" in record &&
      typeof record.active === "boolean" &&
      "estimated" in record &&
      typeof record.estimated === "boolean" &&
      "rate" in record &&
      typeof record.rate === "number" &&
      Number.isFinite(record.rate) &&
      record.rate >= 0 &&
      "outputTokens" in record &&
      typeof record.outputTokens === "number" &&
      Number.isFinite(record.outputTokens) &&
      record.outputTokens >= 0 &&
      Object.keys(record).every((key) => Object.hasOwn(recordKeys, key))
    );
  });
}

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
    const changed = new Set<string>();
    if (
      typeof event.snapshot !== "boolean" ||
      !Array.isArray(event.removed) ||
      event.removed.length > TOKEN_RATE_MAX_ENTRIES ||
      (event.snapshot && event.removed.length !== 0) ||
      !isTokenRateSnapshot(event, changed)
    )
      return false;
    if (event.snapshot) {
      replaceWorkspaceTokenRates(event);
      baselineEpoch = event.epoch;
      baselineWorkspaceId = event.workspaceId;
      return true;
    }
    const removed = new Set<string>();
    const records = index.get();
    let size = records.size;
    for (const id of event.removed) {
      if (!validIdentifier(id) || removed.has(id) || changed.has(id))
        return false;
      removed.add(id);
      if (records.has(id)) size--;
    }
    for (const record of event.records)
      if (!records.has(record.agentId)) size++;
    if (size > TOKEN_RATE_MAX_ENTRIES) return false;
    const next = new Map(records);
    for (const id of event.removed) next.delete(id);
    for (const record of event.records) next.set(record.agentId, record);
    mirror.apply({ records: [...next.values()] });
    return true;
  }

  // Materialize the merged baseline only when a late local or peer subscriber asks.
  function getWorkspaceTokenRates(): components["schemas"]["TokenRateSnapshot"] {
    return { records: mirror.current.records.map((record) => ({ ...record })) };
  }

  function receiveWorkspaceTokenRates(value: unknown) {
    if (!isTokenRateSnapshot(value)) return false;
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
