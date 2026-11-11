import { makeAutoObservable, observableRef } from "mobx";
import { errorText, type GetOptions, type GetResult } from "../api.ts";
import { AnalyticsScope as GeneratedApiAnalyticsScope } from "../generated/api.ts";
import type { Net } from "../servers/net/net.ts";
import { resourceData, type ReadSlot } from "../store/runtime.ts";
import { bytes, count, finite } from "./format.ts";
export type AnalyticsResult = GetResult<"/api/analytics">;
export type AnalyticsUsageRecord = NonNullable<
  AnalyticsResult["timeline"]
>[number];
export type AnalyticsItemRecord = NonNullable<
  AnalyticsResult["itemRecords"]
>[number];
export type AnalyticsTokenGroup =
  | NonNullable<AnalyticsResult["agentTotals"]>[number]
  | NonNullable<AnalyticsResult["modelTotals"]>[number]
  | NonNullable<AnalyticsResult["accountTotals"]>[number];
export type AnalyticsPayload = NonNullable<AnalyticsItemRecord["input"]>;
export type MetricRow = readonly [label: string, value: string, hint: string];
export type AnalyticsContextPoint = AnalyticsUsageRecord & {
  last: AnalyticsUsageRecord["last"] & { totalTokens: number };
  modelContextWindow: number;
};
export type AnalyticsSelection = {
  query: AnalyticsQuery;
  read: ReadSlot<AnalyticsResult>;
};
export type AnalyticsQuery = NonNullable<GetOptions<"/api/analytics">["query"]>;

type AnalyticsNet = Pick<Net, "exportAnalytics"> & {
  reads: Pick<Net["reads"], "analytics">;
};
const tokenFields = [
  ["inputTokens", "Input", "Includes cached input"],
  ["cachedInputTokens", "Cached input", "Subset of input"],
  ["cacheWriteInputTokens", "Cache write", "Only when reported"],
  ["outputTokens", "Output", "Includes reasoning output"],
  ["reasoningOutputTokens", "Reasoning output", "Subset of output"],
  ["totalTokens", "Total", "Provider-reported total"],
] as const;

export class AnalyticsStore {
  query: AnalyticsQuery;
  read: ReadSlot<AnalyticsResult>;
  selected: string | null = null;
  tab: string | null = "overview";
  error = "";
  previousTools: string[] = [];
  details = {
    rateLimits: [] as ReadSlot<AnalyticsResult>[],
    turns: [] as ReadSlot<AnalyticsResult>[],
  };
  constructor(selection: AnalyticsSelection) {
    this.query = selection.query;
    this.read = selection.read;
    makeAutoObservable(
      this,
      { read: observableRef, details: observableRef },
      { autoBind: true },
    );
  }
  get metrics(): Record<"tokens" | "secondary" | "payload", MetricRow[]> {
    const s = this.data?.summary;
    return {
      tokens: tokenFields.map(([key, label, hint]) => {
        const observations = s?.tokenObservations?.[key],
          samples = s?.usageSamples;
        return [
          label,
          count(s?.tokens?.[key]),
          finite(observations) && finite(samples) && samples > 0
            ? `${hint}. ${observations < samples ? `Partial: ${count(observations)} of ${count(samples)} reports` : `All ${count(samples)} reports`}`
            : hint,
        ];
      }),
      secondary: [
        [
          "Cache hit rate",
          finite(s?.cacheHitRate)
            ? `${(s.cacheHitRate * 100).toFixed(1)}%`
            : "Unavailable",
          "Cached input / input",
        ],
        [
          "Peak context",
          finite(s?.peakContextPercent)
            ? `${s.peakContextPercent.toFixed(1)}%`
            : "Unavailable",
          `${count(s?.peakContextTokens)} tokens`,
        ],
        ["Compactions", count(s?.compactions), "Recorded events"],
        [
          "Agents / turns",
          `${count(s?.agents)} / ${count(s?.turns)}`,
          `${count(s?.usageSamples)} usage reports`,
        ],
      ],
      payload: [
        [
          "Model input payload",
          bytes(s?.modelInputBytes),
          "Recorded tool arguments",
        ],
        [
          "Model result payload",
          bytes(s?.modelOutputBytes),
          "Tool results returned to the model",
        ],
        [
          "Model tool calls",
          count(s?.modelToolCalls),
          `${count(s?.modelFailedToolCalls)} reported failures`,
        ],
        [
          "Native tool calls",
          count(s?.protocolToolCalls),
          `${count(s?.protocolFailedToolCalls)} reported failures`,
        ],
      ],
    };
  }
  get data() {
    return resourceData(this.read.current);
  }
  get period() {
    return this.query.from_time && this.query.to_time
      ? String(this.query.to_time - this.query.from_time)
      : "all";
  }
  get busy() {
    return this.read.current.status === "loading";
  }
  get olderDetails() {
    return {
      rateLimits: this.details.rateLimits.flatMap(
        (slot) => resourceData(slot.current)?.rateLimits ?? [],
      ),
      turns: this.details.turns.flatMap(
        (slot) => resourceData(slot.current)?.turns ?? [],
      ),
    };
  }
  get loadingDetails() {
    return {
      rateLimits: this.details.rateLimits.some(
        (slot) => slot.current.status === "loading",
      ),
      turns: this.details.turns.some(
        (slot) => slot.current.status === "loading",
      ),
    };
  }
  get knownTools() {
    return [
      ...new Set([
        ...this.previousTools,
        ...(this.data?.tools ?? []).map((tool) => tool.name),
      ]),
    ].sort();
  }
  get readError() {
    return this.read.current.status === "error"
      ? this.read.current.error
      : [...this.details.turns, ...this.details.rateLimits]
          .map((slot) => slot.current)
          .find((state) => state.status === "error")?.error;
  }
  get tools() {
    return [...(this.data?.tools ?? [])].sort(
      (a, b) => (b.outputBytes ?? 0) - (a.outputBytes ?? 0),
    );
  }
  get rankings() {
    return [true, false].map((model) => {
      const rows = this.tools
        .filter(
          (tool) =>
            (tool.payloadBoundary === "model" ||
              tool.type === "modelToolCall") === model,
        )
        .map((tool) => ({
          ...tool,
          input: model ? tool.modelInputBytes : tool.inputBytes,
          output: model ? tool.modelOutputBytes : tool.outputBytes,
        }))
        .sort((a, b) => (b.output ?? 0) - (a.output ?? 0));
      const maximum = Math.max(1, ...rows.map((row) => row.output ?? 0));
      return {
        title: model ? "Model-visible tool results" : "Native protocol output",
        rows: rows.map((row) => ({
          ...row,
          width: ((row.output ?? 0) / maximum) * 100 + "%",
        })),
      };
    });
  }
  get selectedCall() {
    return this.data?.calls?.find((call) => call.id === this.selected);
  }
  get chart() {
    return contextChart(
      this.data?.chartBuckets ?? this.data?.timeline ?? [],
      (this.query.scope ?? GeneratedApiAnalyticsScope.agent) !==
        GeneratedApiAnalyticsScope.agent,
    );
  }
  get turns() {
    return [...(this.data?.turns ?? []), ...this.olderDetails.turns].sort(
      (a, b) => (b.startedAt ?? 0) - (a.startedAt ?? 0),
    );
  }
  get rateLimits() {
    return [
      ...(this.data?.rateLimits ?? []),
      ...this.olderDetails.rateLimits,
    ].sort((a, b) => (b.at ?? 0) - (a.at ?? 0));
  }
  get monitors() {
    return [...(this.data?.operations?.monitors ?? [])]
      .sort((a, b) => (b.created ?? 0) - (a.created ?? 0))
      .slice(0, 100);
  }
  get notifications() {
    const groups: Record<string, { count: number; bytes: number }> = {};
    for (const row of this.data?.notifications ?? []) {
      const group = (groups[row.method] ??= { count: 0, bytes: 0 });
      group.count += row.count;
      group.bytes += row.bytes;
    }
    return Object.entries(groups);
  }
  setSelected(value: string | null) {
    this.selected = value;
  }
  setTab(value: string | null) {
    this.tab = value;
  }
  setKnownTools(value: string[]) {
    this.previousTools = value;
  }
  loadOlderDetails(net: AnalyticsNet, detail: "rateLimits" | "turns") {
    if (!this.data) return;
    const read = net.reads.analytics({
      ...this.query,
      view: "detail",
      detail,
      limit: 100,
      offset:
        (this.data[detail]?.length ?? 0) + this.olderDetails[detail].length,
    });
    if (!this.details[detail].includes(read))
      this.details = {
        ...this.details,
        [detail]: [...this.details[detail], read],
      };
    void read.read().catch(() => {});
  }
  load(net: AnalyticsNet, patch: Partial<AnalyticsQuery> = {}) {
    this.previousTools = this.knownTools;
    this.query = { ...this.query, offset: 0, ...patch };
    this.read = net.reads.analytics(this.query);
    this.selected = null;
    this.details = { rateLimits: [], turns: [] };
    this.error = "";
    void this.read.read().catch(() => {});
  }
  timeRange(value = this.period) {
    const now = Math.floor(Date.now() / 1000);
    return {
      to_time: now,
      from_time: value === "all" ? undefined : now - Number(value),
    };
  }
  *download(
    net: AnalyticsNet,
  ): Generator<
    ReturnType<AnalyticsNet["exportAnalytics"]>,
    void,
    Awaited<ReturnType<AnalyticsNet["exportAnalytics"]>>
  > {
    this.error = "";
    const query = this.query;
    const scope = query.scope ?? "agent";
    try {
      const { blob } = yield net.exportAnalytics({
        ...query,
        export: "1",
      });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download =
        "codex-studio-analytics-" +
        scope +
        "-" +
        new Date().toISOString().replaceAll(":", "-") +
        ".json";
      anchor.click();
      setTimeout(() => URL.revokeObjectURL(url), 60000);
    } catch (failure) {
      this.error = errorText(failure);
    }
  }
}
export function contextChart(
  timeline: AnalyticsUsageRecord[],
  aggregate: boolean,
) {
  const points = timeline
    .filter(
      (p): p is AnalyticsContextPoint =>
        finite(p.at) &&
        finite(p.last?.totalTokens) &&
        finite(p.modelContextWindow) &&
        p.modelContextWindow > 0,
    )
    .sort((a, b) => a.at - b.at);
  if (!points.length) return null;
  const start = points[0]!.at,
    end = points.at(-1)!.at;
  const groups = new Map<string, AnalyticsContextPoint[]>();
  points.forEach((p) => {
    const id = aggregate ? "Selected agents" : p.agentId;
    groups.set(id, [...(groups.get(id) || []), p]);
  });
  const peak = Math.max(
    100,
    ...points.map((p) => (p.last.totalTokens / p.modelContextWindow) * 100),
  );
  const xAt = (at: number) => 42 + ((at - start) / (end - start || 1)) * 906;
  const x = (p: AnalyticsContextPoint) => xAt(p.at);
  const y = (p: AnalyticsContextPoint) =>
    152 - (((p.last.totalTokens / p.modelContextWindow) * 100) / peak) * 136;

  return {
    points,
    start,
    end,
    groups: [...groups],
    peak,
    xAt,
    x,
    y,
    aggregate,
  };
}
export type ContextChartData = ReturnType<typeof contextChart>;
