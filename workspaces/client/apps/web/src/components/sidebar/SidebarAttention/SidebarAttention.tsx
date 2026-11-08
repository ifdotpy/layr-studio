import type { ReactNode } from "react";
import { observer } from "mobx-react-lite";
import type { SidebarChatView } from "../support/viewModel.ts";
import { ChatIndicatorKind as ChatOperationalKind } from "../../../status/operationalStatus.ts";
export { default as SidebarProjectStates } from "../ProjectStateCounts/ProjectStateCounts.tsx";

export const SidebarAttention = observer(function SidebarAttention({
  rows,
  renderRow,
}: {
  rows: readonly SidebarChatView[];
  renderRow: (row: SidebarChatView, priority: boolean) => ReactNode;
}) {
  const attention = rows.filter(
    (row) =>
      row.indicator.kind === ChatOperationalKind.answer ||
      row.indicator.kind === ChatOperationalKind.error,
  );
  // This lane's presentation preserves answer-before-error urgency.
  const ordered = [
    ...attention.filter(
      (row) => row.indicator.kind === ChatOperationalKind.answer,
    ),
    ...attention.filter(
      (row) => row.indicator.kind === ChatOperationalKind.error,
    ),
  ];
  return ordered.length ? (
    <section className="fleet-attention" aria-label="Needs you">
      <h2 className="fleet-lane-heading">
        Needs you <span>{ordered.length}</span>
      </h2>
      {ordered.map((row) => renderRow(row, true))}
    </section>
  ) : null;
});
