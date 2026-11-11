import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  ActionIcon,
  Button,
  Loader,
  Modal,
  NativeSelect,
  Tabs,
  Tooltip,
} from "@mantine/core";
import {
  ArrowDownToLine,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment, type ReactNode } from "react";
import { errorText } from "../../../api.ts";
import { AnalyticsScope as GeneratedApiAnalyticsScope } from "../../../generated/api.ts";
import { type Agent } from "../../../types.ts";
import {
  useServerNet,
  useServerOwner,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import "./Analytics.css";
import {
  AnalyticsStore,
  type AnalyticsPayload,
  type AnalyticsQuery,
  type AnalyticsSelection,
  type AnalyticsTokenGroup,
  type ContextChartData,
  type MetricRow,
} from "../../../analytics/AnalyticsStore.ts";
import {
  bytes,
  count,
  duration,
  finite,
  shortTime,
  time,
} from "../../../analytics/format.ts";
export type { AnalyticsSelection } from "../../../analytics/AnalyticsStore.ts";

const colors = [
  "light-dark(#7044bc, #a59bff)",
  "light-dark(#187c70, #70c9bd)",
  "light-dark(#8a5700, #e2b479)",
  "light-dark(#ab3266, #ec91b2)",
  "light-dark(#286fbf, #82b4ef)",
  "light-dark(#65751f, #c1ca81)",
];
function Metrics({
  rows,
  secondary = false,
}: {
  rows: readonly MetricRow[];
  secondary?: boolean;
}) {
  return (
    <div className={secondary ? "analytics-secondary" : "analytics-metrics"}>
      {rows.map(([label, value, hint]) => (
        <div className="analytics-metric" key={label}>
          <span>{label}</span>
          <strong>{value}</strong>
          {hint && <small>{hint}</small>}
        </div>
      ))}
    </div>
  );
}
function Raw({
  value,
  label = "Recorded metadata",
}: {
  value: unknown;
  label?: string;
}) {
  return (
    <details className="analytics-raw">
      <summary>{label}</summary>
      <pre>{JSON.stringify(value, null, 2)}</pre>
    </details>
  );
}
function ContextChart({
  chart,
  compactions,
}: {
  chart: ContextChartData;
  compactions: NonNullable<AnalyticsStore["data"]>["compactions"];
}) {
  if (!chart)
    return (
      <div className="analytics-empty">
        No context-window measurements in this period.
      </div>
    );
  const { points, start, end, groups, peak, xAt, x, y, aggregate } = chart;
  return (
    <>
      <svg
        className="analytics-chart"
        viewBox="0 0 970 182"
        role="img"
        aria-label={`Context usage over time, ${points.length} observations across ${groups.length} agents`}
      >
        {[0, 50, 100].map((n) => (
          <g key={n}>
            <line
              x1="42"
              x2="948"
              y1={152 - (n / peak) * 136}
              y2={152 - (n / peak) * 136}
            />
            <text x="3" y={156 - (n / peak) * 136}>
              {n}%
            </text>
          </g>
        ))}
        {(compactions ?? [])
          .filter((c) => finite(c.at) && c.at >= start && c.at <= end)
          .map((c) => (
            <line
              key={c.id}
              className="analytics-compaction-marker"
              x1={xAt(c.at)}
              x2={xAt(c.at)}
              y1="12"
              y2="152"
            >
              <title>{`Compaction · ${c.agentName || c.agentId} · ${time(c.at)}`}</title>
            </line>
          ))}
        {groups.map(([id, group], i) => (
          <g key={id} style={{ color: colors[i % colors.length] }}>
            <polyline
              points={group.map((p) => `${x(p)},${y(p)}`).join(" ")}
              fill="none"
              stroke="currentColor"
              strokeWidth="1.8"
            />
            {group.map((p) => (
              <circle key={p.id} cx={x(p)} cy={y(p)} r="3" fill="currentColor">
                <title>{`${p.agentName || id}: ${count(p.last.totalTokens)} / ${count(p.modelContextWindow)} tokens (${((p.last.totalTokens / p.modelContextWindow) * 100).toFixed(1)}%)\n${time(p.at)}`}</title>
              </circle>
            ))}
          </g>
        ))}
        <text x="42" y="176">
          {shortTime(start)}
        </text>
        <text x="948" y="176" textAnchor="end">
          {shortTime(end)}
        </text>
      </svg>
      <div className="analytics-legend">
        {groups.map(([id, group], i) => (
          <span key={id}>
            <i style={{ background: colors[i % colors.length] }} />
            {aggregate ? id : group[0]?.agentName || id}
          </span>
        ))}
      </div>
    </>
  );
}
const tokenColumns = [
  "inputTokens",
  "cachedInputTokens",
  "outputTokens",
  "totalTokens",
] as const;
function TokenGroups<
  T extends AnalyticsTokenGroup,
  K extends keyof T & string,
>(props: { title: string; rows: T[]; identity: K; detail?: boolean }) {
  return (
    <Section title={props.title} description="Provider token use">
      {!props.rows.length ? (
        <p className="analytics-note">No usage records.</p>
      ) : (
        <DataTable
          headings={[
            props.title === "Agents"
              ? "Agent"
              : props.title === "Models"
                ? "Model"
                : "Account",
            "Input",
            "Cached input",
            "Output",
            "Total",
            ...(props.detail ? ["Calls", "Failed", "Compactions"] : []),
          ]}
          rows={props.rows.map((row, index) => [
            String(row[props.identity]) + ":" + index,
            <>
              {typeof row[props.identity] === "string"
                ? String(row[props.identity])
                : "Unavailable"}
              {props.detail && <Raw value={row} label="Details" />}
            </>,
            ...tokenColumns.map((field) => count(row.tokens?.[field])),
            ...(props.detail && "toolCalls" in row
              ? [row.toolCalls, row.failedToolCalls, row.compactions].map(count)
              : []),
          ])}
        />
      )}
    </Section>
  );
}
function CountGroup({
  title,
  values,
}: {
  title: string;
  values: Record<string, number>;
}) {
  return (
    <div>
      <h4>{title}</h4>
      <Fields
        values={Object.entries(values).map(([key, value]) => [
          key,
          count(value),
        ])}
      />
      {!Object.keys(values).length && (
        <p className="analytics-note">No records.</p>
      )}
    </div>
  );
}
function ToolRanking(props: {
  ranking: AnalyticsStore["rankings"][number];
  onSelect: (name: string) => void;
}) {
  return (
    <Section title={props.ranking.title} description="Ranked by output bytes">
      {!props.ranking.rows.length ? (
        <p className="analytics-note">No measurements recorded.</p>
      ) : (
        <DataTable
          headings={["Tool", "Calls", "Input", "Output", "Duration", "Failed"]}
          rows={props.ranking.rows.map((row) => [
            row.type + ":" + row.name,
            <>
              <button
                className="analytics-tool-name"
                onClick={() => props.onSelect(row.name)}
              >
                {row.name}
              </button>
              <div className="analytics-bar">
                <i style={{ width: row.width }} />
              </div>
            </>,
            count(row.calls),
            bytes(row.input),
            bytes(row.output),
            duration(row.durationMs),
            count(row.failed),
          ])}
        />
      )}
    </Section>
  );
}
function Fields(props: {
  values: readonly (readonly [string, ReactNode])[];
  className?: string;
  flat?: boolean;
}) {
  const Wrap = props.flat ? Fragment : "div";
  return (
    <dl className={props.className}>
      {props.values.map(([label, value]) => (
        <Wrap key={label}>
          <dt>{label}</dt>
          <dd>{value || "Unavailable"}</dd>
        </Wrap>
      ))}
    </dl>
  );
}
function Payload({
  title,
  data,
}: {
  title: string;
  data?: AnalyticsPayload | null | undefined;
}) {
  return (
    <div className="analytics-payload">
      <h4>{title}</h4>
      <Fields
        flat
        values={Object.entries({
          "UTF-8 bytes": count(data?.bytes),
          Characters: count(data?.chars),
          Lines: count(data?.lines),
          Images: count(data?.imageCount),
          "Image bytes": bytes(data?.imageBytes),
        })}
      />
    </div>
  );
}
function Section({
  title,
  description,
  children,
}: {
  title: string;
  description: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="analytics-section">
      <header>
        <h3>{title}</h3>
        <span>{description}</span>
      </header>
      {children}
    </section>
  );
}
function DataTable(props: {
  headings: string[];
  rows: [string, ...ReactNode[]][];
}) {
  return (
    <div className="analytics-table-scroll">
      <table className="analytics-table">
        <thead>
          <tr>
            {props.headings.map((heading) => (
              <th key={heading}>{heading}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {props.rows.map(([key, ...cells]) => (
            <tr key={key}>
              {cells.map((cell, index) => (
                <td key={index}>{cell}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
function Records<T>(props: {
  rows: readonly T[];
  identify: (row: T, index: number) => string | number | null;
  summary: (row: T) => ReactNode[];
  raw?: (row: T) => unknown;
  label?: string;
  children?: (row: T) => ReactNode;
}) {
  return (
    <div className="analytics-observations">
      {props.rows.map((row, index) => {
        const [name, at, value, detail] = props.summary(row);
        return (
          <details key={props.identify(row, index)}>
            <summary>
              <span>
                {name}
                {at && <small>{at}</small>}
              </span>
              <span>
                {value}
                {detail && <small>{detail}</small>}
              </span>
            </summary>
            {props.children ? (
              props.children(row)
            ) : (
              <Raw
                value={props.raw ? props.raw(row) : row}
                {...(props.label ? { label: props.label } : {})}
              />
            )}
          </details>
        );
      })}
    </div>
  );
}
type AnalyticsPanelProps = { model: AnalyticsStore };
const Overview = observer(function Overview({ model }: AnalyticsPanelProps) {
  const { data, setTab } = model;
  if (!data) return null;
  const s = data.summary;
  return (
    <>
      {(finite(s?.exactResponseSamples) || finite(s?.legacyUsageSamples)) && (
        <p className="analytics-note">
          Token evidence: {count(s?.exactResponseSamples)} identified response
          records; {count(s?.legacyUsageSamples)} legacy usage records.
        </p>
      )}
      <Metrics rows={model.metrics.tokens} />
      <Metrics rows={model.metrics.secondary} secondary />
      <Section
        title="Context over time"
        description="Peak report per time bucket / model window"
      >
        <ContextChart
          chart={model.chart}
          compactions={data.compactions || []}
        />
        <p className="analytics-note">
          Each point is the highest context report in its time bucket. Dotted
          lines mark compactions.
        </p>
      </Section>
      <section className="analytics-section">
        <header>
          <h3>Tool payloads</h3>
          <Button
            variant="subtle"
            size="compact-xs"
            onClick={() => setTab("tools")}
          >
            Inspect calls
          </Button>
        </header>
        <Metrics rows={model.metrics.payload} secondary />
        <p className="analytics-note">
          Model tool time: {duration(s?.modelDurationMs)}. Native tool time:{" "}
          {duration(s?.protocolDurationMs)}. Each value sums measured calls
          within its group; parallel calls can overlap.
        </p>
      </section>
    </>
  );
});
const Calls = observer(function Calls({ model }: AnalyticsPanelProps) {
  const { data, busy, selected, tools, selectedCall, setSelected } = model;
  const net = useServerNet();
  const load = (patch: Partial<AnalyticsQuery>) => model.load(net, patch);
  if (!data) return null;
  const { offset = 0 } = model.query;
  const calls = data.calls ?? [],
    total = data.pagination?.total ?? 0;
  return (
    <>
      {model.rankings.map((ranking) => (
        <ToolRanking
          key={ranking.title}
          ranking={ranking}
          onSelect={(name) => load({ tool: name })}
        />
      ))}
      <Raw
        value={tools}
        label="Tool measurement coverage and duration distributions"
      />
      <p className="analytics-note">
        These groups overlap. Do not add them together.
      </p>
      <Section
        title="Call records"
        description={<>{count(total)} model and native records</>}
      >
        <div className="analytics-calls">
          {calls.map((c) => (
            <div
              key={c.id}
              className={`analytics-call ${selected === c.id ? "is-selected" : ""}`}
            >
              <button
                className="analytics-call-row"
                aria-expanded={selected === c.id}
                onClick={() => setSelected(selected === c.id ? null : c.id)}
              >
                <span className="analytics-call-main">
                  <strong>{c.name}</strong>
                  <small>
                    {c.agentName || c.agentId} · {time(c.startedAt)} ·{" "}
                    {c.payloadBoundary === "model"
                      ? "Model-visible"
                      : "Native protocol"}
                  </small>
                </span>
                <span
                  className={`analytics-status ${["failed", "error"].includes(c.status) ? "is-failed" : ""}`}
                >
                  {c.status}
                </span>
                <span>
                  {bytes(
                    c.payloadBoundary === "model"
                      ? c.modelOutput?.bytes
                      : c.output?.bytes,
                  )}
                </span>
                <span>{duration(c.durationMs)}</span>
              </button>
              {selected === c.id && selectedCall && (
                <div className="analytics-call-detail">
                  <Fields
                    className="analytics-identities"
                    values={[
                      ["Call", c.id],
                      ["Agent", c.agentId],
                      ["Thread", c.threadId],
                      ["Turn", c.turnId],
                      ["Model", c.model],
                      ["Account", c.accountKey],
                      ["Source", c.source],
                      ["Started", time(c.startedAt)],
                      ["Finished", time(c.finishedAt)],
                    ]}
                  />
                  {c.command && (
                    <pre className="analytics-command">
                      {Array.isArray(c.command)
                        ? c.command.join(" ")
                        : c.command}
                    </pre>
                  )}
                  {c.error && (
                    <p className="analytics-error">
                      {typeof c.error === "string"
                        ? c.error
                        : JSON.stringify(c.error)}
                    </p>
                  )}
                  {c.payloadBoundary === "model" ||
                  c.modelInput ||
                  c.modelOutput ? (
                    <div className="analytics-payloads">
                      <Payload
                        title="Model tool arguments"
                        data={c.modelInput}
                      />
                      <Payload
                        title="Model-visible result"
                        data={c.modelOutput}
                      />
                    </div>
                  ) : null}
                  {c.payloadBoundary !== "model" && (
                    <div className="analytics-payloads">
                      <Payload title="Protocol input" data={c.input} />
                      <Payload title="Protocol output" data={c.output} />
                    </div>
                  )}
                  <Raw value={c} />
                </div>
              )}
            </div>
          ))}
        </div>
        {!calls.length && <p className="analytics-empty">No call records.</p>}
        <div className="analytics-pagination">
          <span>
            {total
              ? `${offset + 1} to ${Math.min(offset + calls.length, total)} of ${count(total)}`
              : "0 calls"}
          </span>
          <Button
            size="compact-xs"
            variant="subtle"
            leftSection={<ChevronLeft size={14} />}
            disabled={busy || offset === 0}
            onClick={() => {
              const next = Math.max(0, offset - 30);
              load({ offset: next });
            }}
          >
            Previous
          </Button>
          <Button
            size="compact-xs"
            variant="subtle"
            rightSection={<ChevronRight size={14} />}
            disabled={busy || !data.pagination?.hasMore}
            onClick={() => {
              load({ offset: offset + 30 });
            }}
          >
            Next
          </Button>
        </div>
      </Section>
    </>
  );
});
const Activity = observer(function Activity({ model }: AnalyticsPanelProps) {
  const data = model.data;
  if (!data) return null;
  const timeline = data.timeline ?? [];
  return (
    <>
      <TokenGroups
        title="Agents"
        rows={data.agentTotals ?? []}
        identity="name"
        detail
      />
      <TokenGroups
        title="Models"
        rows={data.modelTotals ?? []}
        identity="model"
      />
      <TokenGroups
        title="Accounts"
        rows={data.accountTotals ?? []}
        identity="accountKey"
      />
      <Section
        title="Turns"
        description={`Showing ${count(model.turns.length)} of ${count(data.detailPagination?.turns?.total ?? 0)} turns; elapsed time includes tools and waits`}
      >
        <Records
          rows={model.turns}
          identify={(turn) => turn.agentId + ":" + turn.turnId}
          summary={(turn) => [
            turn.agentName || turn.agentId,
            `${time(turn.startedAt)} · ${turn.status}`,
            duration(turn.durationMs),
            `First output ${duration(turn.firstOutputDelayMs)}`,
          ]}
        />
        {!data.turns?.length && (
          <p className="analytics-note">No turn measurements recorded.</p>
        )}
        <Older model={model} detail="turns" />
      </Section>
      <Section
        title="Background activity"
        description="Latest 100 monitors and event delivery"
      >
        <div className="analytics-operation-counts">
          <CountGroup
            title="Events"
            values={data.operations?.eventCounts ?? {}}
          />
          <CountGroup
            title="Approval states"
            values={data.operations?.approvalCounts ?? {}}
          />
        </div>
        <Records
          rows={model.monitors}
          identify={(row) => row.id}
          summary={(row) => [
            row.id,
            `${row.agent} · ${time(row.created)}`,
            row.status,
            `${bytes(row.bytes)} · Exit ${count(row.exitCode)}`,
          ]}
        />
      </Section>
      <Section title="Protocol events" description="Grouped by UTC hour">
        <DataTable
          headings={["Method", "Events", "Bytes"]}
          rows={model.notifications.map(([method, row]) => [
            method,
            method,
            count(row.count),
            bytes(row.bytes),
          ])}
        />
        <Raw
          value={data.notifications ?? []}
          label="Event buckets by agent and hour"
        />
      </Section>
      <Section
        title="Account limit history"
        description={`Showing ${count(model.rateLimits.length)} of ${count(data.detailPagination?.rateLimits?.total ?? 0)} allowance snapshots`}
      >
        <Records
          rows={model.rateLimits}
          identify={(row, index) => row.accountKey + ":" + row.at + ":" + index}
          summary={(row) => [row.accountKey, "", time(row.at)]}
          raw={(row) => row.data}
          label="Recorded allowance"
        />
        {!data.rateLimits?.length && (
          <p className="analytics-note">No allowance snapshots recorded.</p>
        )}
        <Older model={model} detail="rateLimits" />
      </Section>
      <Section
        title="Compactions"
        description={`${count(data.summary?.compactions)} recorded; latest 100 shown`}
      >
        <Records
          rows={(data.compactions ?? []).slice(0, 100)}
          identify={(row) => row.id}
          summary={(row) => [
            row.agentName || row.agentId,
            time(row.at),
            "Compaction",
          ]}
        />
        {!data.compactions?.length && (
          <p className="analytics-note">
            No compaction records available in this period.
          </p>
        )}
      </Section>
      <Section
        title="Recorded item types"
        description="Messages, reasoning, tools and other observed items"
      >
        <DataTable
          headings={["Type", "Items", "Bytes", "Characters"]}
          rows={(data.items ?? []).map((row) => [
            row.type,
            row.type,
            count(row.count),
            bytes(row.bytes),
            count(row.chars),
          ])}
        />
        {!data.items?.length && (
          <p className="analytics-empty">No item measurements recorded.</p>
        )}
      </Section>
      <Section
        title="Usage reports"
        description={`${timeline.length} recent observations`}
      >
        <Records
          rows={[...timeline].reverse()}
          identify={(row) => row.id}
          summary={(row) => [
            row.agentName || row.agentId,
            `${time(row.at)} · ${row.source}`,
            `${count(row.last?.totalTokens)} tokens`,
            row.reset ? "Cumulative counter reset" : "",
          ]}
        >
          {(row) => (
            <div>
              <Fields
                className="analytics-identities"
                values={[
                  ["Turn", row.turnId],
                  ["Model", row.model],
                ]}
              />
              <Raw value={row} label="Provider report and recorded metadata" />
            </div>
          )}
        </Records>
        {!timeline.length && (
          <p className="analytics-empty">No provider usage reports recorded.</p>
        )}
      </Section>
    </>
  );
});

const Older = observer(function Older({
  model,
  detail,
}: {
  model: AnalyticsStore;
  detail: "turns" | "rateLimits";
}) {
  const net = useServerNet();
  const total = model.data?.detailPagination?.[detail]?.total ?? 0;
  const loaded =
    (model.data?.[detail]?.length ?? 0) + model.olderDetails[detail].length;
  return (
    loaded < total && (
      <Button
        size="compact-xs"
        variant="subtle"
        loading={model.loadingDetails[detail]}
        onClick={() => model.loadOlderDetails(net, detail)}
      >
        Load 100 older {detail === "turns" ? "turns" : "snapshots"}
      </Button>
    )
  );
});

const analyticsScopes = Object.values(GeneratedApiAnalyticsScope);
const isAnalyticsScope = (
  value: string,
): value is NonNullable<AnalyticsSelection["query"]["scope"]> =>
  analyticsScopes.some((scope) => scope === value);
type AnalyticsProps = {
  agent: Agent;
  selection: AnalyticsSelection;
  onClose: () => void;
};
const readKeys = new WeakMap<AnalyticsSelection["read"], string>();
function readKey(read: AnalyticsSelection["read"]) {
  let key = readKeys.get(read);
  if (!key) {
    key = randomUUID();
    readKeys.set(read, key);
  }
  return key;
}
export default function Analytics(props: AnalyticsProps) {
  const owner = useServerOwner(),
    net = useServerNet();
  const { agent, onClose } = props;
  return (
    <Owned
      key={JSON.stringify([
        owner?.id,
        props.selection.query,
        readKey(props.selection.read),
      ])}
      create={() => new AnalyticsStore(props.selection)}
    >
      {(model) => {
        const {
          period,
          data,
          error,
          busy,
          tab,
          knownTools,
          readError,
          timeRange,
          setTab,
          setKnownTools,
        } = model;
        const load = (patch: Partial<AnalyticsQuery> = {}) =>
          model.load(net, patch);
        const download = () => model.download(net);
        const { scope = "agent", tool = "" } = model.query;
        const s = data?.summary;
        return (
          <Modal
            opened
            onClose={onClose}
            title="Context analytics"
            size="min(1160px, calc(100vw - 24px))"
            className="analytics-modal"
            styles={{ body: { padding: 0 }, content: { overflow: "hidden" } }}
          >
            <div className="analytics-root" aria-busy={busy}>
              <div className="analytics-toolbar">
                <NativeSelect
                  label="Scope"
                  aria-label="Analytics scope"
                  value={scope}
                  onChange={(e) => {
                    if (!isAnalyticsScope(e.currentTarget.value)) return;
                    load({
                      ...timeRange(),
                      scope: e.currentTarget.value,
                      tool: undefined,
                    });
                    setKnownTools([]);
                  }}
                  data={[
                    { value: "agent", label: "This agent" },
                    { value: "team", label: "Whole team" },
                    { value: "all", label: "All agents" },
                  ]}
                />
                <NativeSelect
                  label="Period"
                  aria-label="Analytics period"
                  value={period}
                  onChange={(e) => load(timeRange(e.currentTarget.value))}
                  data={[
                    { value: "all", label: "All recorded history" },
                    { value: "3600", label: "Last hour" },
                    { value: "86400", label: "Last 24 hours" },
                    { value: "604800", label: "Last 7 days" },
                    { value: "2592000", label: "Last 30 days" },
                  ]}
                />
                <NativeSelect
                  label="Tool calls"
                  aria-label="Analytics tool"
                  value={tool}
                  onChange={(e) => {
                    load({ tool: e.currentTarget.value || undefined });
                    setTab("tools");
                  }}
                  data={[
                    { value: "", label: "All tools" },
                    ...knownTools.map((name) => ({ value: name, label: name })),
                  ]}
                />
                <div className="analytics-toolbar-actions">
                  <Tooltip label="Refresh analytics">
                    <ActionIcon
                      aria-label="Refresh analytics"
                      variant="subtle"
                      size="lg"
                      disabled={busy}
                      onClick={() => load(timeRange())}
                    >
                      <RefreshCw size={16} />
                    </ActionIcon>
                  </Tooltip>
                  <Button
                    variant="light"
                    size="xs"
                    leftSection={<ArrowDownToLine size={14} />}
                    disabled={busy || !data}
                    onClick={() => void download()}
                  >
                    Export JSON
                  </Button>
                </div>
              </div>
              <div className="analytics-scope">
                <span>
                  {scope === GeneratedApiAnalyticsScope.agent
                    ? agent.name
                    : scope === "team"
                      ? "Selected team"
                      : "All recorded agents"}
                </span>
                <span>
                  {busy
                    ? "Updating…"
                    : data
                      ? `Updated ${time(data.generatedAt)}`
                      : ""}
                </span>
              </div>
              {(error || readError) && (
                <p className="analytics-error" role="alert">
                  {error || errorText(readError)}
                </p>
              )}
              {busy && !data && (
                <div className="analytics-empty">
                  <Loader size="sm" aria-label="Load analytics" />
                </div>
              )}
              {data && (
                <Tabs value={tab} onChange={setTab} keepMounted={false}>
                  <Tabs.List>
                    <Tabs.Tab value="overview">Overview</Tabs.Tab>
                    <Tabs.Tab value="tools">
                      Tool calls{" "}
                      <span
                        className="analytics-tab-count"
                        title="Model-visible tool calls"
                      >
                        {count(s?.modelToolCalls)}
                      </span>
                    </Tabs.Tab>
                    <Tabs.Tab value="activity">Activity</Tabs.Tab>
                  </Tabs.List>
                  <div className="analytics-scroll">
                    <p className="analytics-definition">
                      Tokens come from provider reports. Tool sizes are in
                      bytes.
                    </p>
                    {(data.coverage?.captureErrors?.count ?? 0) > 0 ||
                    (data.coverage?.historyErrors?.length ?? 0) > 0 ? (
                      <p className="analytics-filter-note" role="status">
                        Collection reported errors. Some measurements may be
                        missing. See Data coverage below.
                      </p>
                    ) : null}
                    {(s?.provisionalUsageSamples || 0) > 0 && (
                      <p className="analytics-filter-note">
                        {count(s?.provisionalUsageSamples)} provisional notices
                        excluded; current requests may appear after history
                        sync. These notices can duplicate known requests.
                      </p>
                    )}
                    {tool && (
                      <p className="analytics-filter-note">
                        The tool filter applies to tool calls only.
                      </p>
                    )}
                    <Tabs.Panel value="overview">
                      <Overview model={model} />
                    </Tabs.Panel>
                    <Tabs.Panel value="tools">
                      <Calls model={model} />
                    </Tabs.Panel>
                    <Tabs.Panel value="activity">
                      <Activity model={model} />
                    </Tabs.Panel>
                    <details className="analytics-coverage">
                      <summary>Data coverage and measurement limits</summary>
                      <p>
                        Collection started {time(data.coverage?.trackingSince)}.
                        Older stored history may have fewer measurements.
                        Missing values stay unavailable.
                      </p>
                      {(data.coverage?.notes || []).map((note, i) => (
                        <p key={i}>{note}</p>
                      ))}
                      <p>
                        Cached input is part of input. Reasoning is part of
                        output.
                      </p>
                      <p>Cost estimates are in Account limits.</p>
                      {(data.coverage?.captureErrors?.count ?? 0) > 0 && (
                        <p className="analytics-error">
                          {count(data.coverage?.captureErrors?.count)}{" "}
                          collection errors recorded. Some data may be missing.
                        </p>
                      )}
                      {Object.entries({
                        "Coverage metadata": data.coverage,
                        "Excluded provisional notices (latest 100)":
                          data.provisionalUsage ?? [],
                        "History import checkpoints": data.history ?? [],
                        "Summary and measurement counts": data.summary,
                      }).map(([label, value]) => (
                        <Raw key={label} label={label} value={value} />
                      ))}
                    </details>
                  </div>
                </Tabs>
              )}
            </div>
          </Modal>
        );
      }}
    </Owned>
  );
}
