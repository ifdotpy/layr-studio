import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { finite as number } from "../../../analytics/format.ts";
import {
  accountSelection,
  dollars,
  type UsageControls,
} from "../Usage/Usage.tsx";

export const UsageCosts = observer(function UsageCosts({
  agent,
  usageStore: ui,
  visible = true,
  opened,
  openRequest,
}: UsageControls) {
  const store = useServerStore();
  const { key } = accountSelection(ui, agent, openRequest);
  const costs =
    visible && opened
      ? resourceData(store.costs().current)?.find(
          (row) => row.accountKey === key,
        )
      : undefined;
  const interestError =
    visible && opened ? ui.interests.get(key)?.error || "" : "";
  return (
    <section className="account-costs" aria-label="Local cost estimates">
      <header>
        <strong>Estimated API cost</strong>
        <span>USD</span>
      </header>
      <div className="account-cost-values">
        {(
          [
            ["Today", costs?.data?.todayUSD],
            ["Last 30 days", costs?.data?.last30DaysUSD],
          ] as const
        ).map(([label, value]) => (
          <div key={label}>
            <span>{label}</span>
            <strong
              className={number(value) ? undefined : "account-cost-missing"}
            >
              {dollars(value)}
            </strong>
          </div>
        ))}
      </div>
      <p className="account-cost-caption" role="status">
        {costs?.refreshing
          ? "Refreshing estimate…"
          : interestError || costs?.error
            ? "Refresh failed · stale estimate"
            : costs?.stale
              ? "Stale estimate"
              : "Saved estimate"}
      </p>
      {number(costs?.at) && (
        <p className="account-cost-caption">
          {`Updated ${Math.max(0, Math.round(Date.now() / 1000 - costs.at))} seconds ago`}
        </p>
      )}
      {(costs?.data?.coverage === "partial" ||
        costs?.stale ||
        costs?.error) && (
        <p className="account-cost-caption">
          {costs?.error || costs?.stale
            ? number(costs?.data?.todayUSD) ||
              number(costs?.data?.last30DaysUSD)
              ? "Saved estimate"
              : "Estimate unavailable"
            : "Partial estimate"}
        </p>
      )}
      {typeof costs?.data?.note === "string" && (
        <p className="account-cost-caption">{costs.data.note}</p>
      )}
    </section>
  );
});
