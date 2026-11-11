import type { AgentDetailEntityDto } from "../../../generated/observableResources.ts";
import { ClaudeRepairReportStore } from "./ClaudeRepairReportStore.ts";
import type { Agent } from "../../../types.ts";
import { missingRepairEvidence, repairReason } from "./claudeRepairModel.ts";

/** Read saved evidence only. This component never starts a session repair or a turn. */
export default function ClaudeRepairNotice({
  agent,
  detail = agent,
  store,
}: {
  agent: Agent;
  store: ClaudeRepairReportStore;
  detail?: AgentDetailEntityDto | Agent | undefined;
}) {
  const current = store.current;
  const repairRequired =
    /CLAUDE_SESSION_REPAIR_REQUIRED|needs repair|journal.*repair/i.test(
      JSON.stringify(detail.error || ""),
    );
  const report = current?.report;
  if ((!report || report.status === "none") && !repairRequired) return null;
  const missing = report ? missingRepairEvidence(report) : [];
  return (
    <aside
      className="claude-repair-notice"
      role="status"
      aria-label="Claude journal repair"
    >
      <strong>
        {report?.status === "repaired"
          ? "Journal repaired"
          : "Journal needs repair"}
      </strong>
      <p>
        {report && report.status !== "none"
          ? repairReason(report)
          : "The bridge cannot confirm the saved session."}
      </p>
      <p>
        Skipped rows:{" "}
        {report && report.status !== "none" ? report.skippedRows : "unknown"}.
      </p>
      <details>
        <summary>Repair report</summary>
        <button type="button" onClick={() => void store.refresh()}>
          Refresh saved report
        </button>
        {report && report.status !== "none" ? (
          <>
            <p>
              This report contains saved evidence. It does not change the
              session.
            </p>
            <dl>
              <dt>Journal sequence</dt>
              <dd>
                {report.invalidSequence
                  ? "Invalid or incomplete"
                  : "No sequence gap reported"}
              </dd>
              <dt>Session and workspace</dt>
              <dd>
                {report.scopeConfirmed === true ? "Confirmed" : "Unconfirmed"}
              </dd>
              <dt>Native history</dt>
              <dd>
                {report.nativeHistoryConfirmed === true
                  ? "Confirmed"
                  : "Unconfirmed"}
              </dd>
            </dl>
            <strong>Missing evidence</strong>
            {missing.length ? (
              <ul>
                {missing.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            ) : (
              <p>No missing evidence was reported.</p>
            )}
          </>
        ) : (
          <p>{current?.error || "No saved report is available."}</p>
        )}
        <p>No repair or replay action is available here.</p>
      </details>
    </aside>
  );
}

export { ClaudeRepairReportStore } from "./ClaudeRepairReportStore.ts";
