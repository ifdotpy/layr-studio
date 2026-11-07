import { observer } from "mobx-react-lite";
import { useAgentDetail } from "./state.ts";
import { ProviderKind } from "../../generated/providerKind.ts";
import { modelProvider } from "../accountPreference.ts";
import type { Agent } from "../../types";
import type { ComponentProps } from "react";
import { ActionButton } from "../ui/primitives";
import {
  INHERITED_MODEL,
  modelDisplayName,
  modelForValue,
} from "../models/modelOption";
import { settingsFor } from "./ExecutionSettings";
import { useWorkerModels } from "./WorkerModelPicker";
import { subagentsOff } from "./SubagentConcurrencyControl";

/** The composer anchor for the full role picker. */
export const ExecutionSummaryTrigger = observer(
  function ExecutionSummaryTrigger({
    agent,
    detail: receipt,
    workerDetail,
    reviewDetail,
    team,
    lead: teamLead,
    onOpen,
    className,
    ...buttonProps
  }: ComponentProps<typeof ActionButton> & {
    agent: Agent;
    detail?: Agent | undefined;
    workerDetail?: Agent | undefined;
    reviewDetail?: Agent | undefined;
    team: Agent[];
    lead?: Agent | null | undefined;
    onOpen: () => void;
  }) {
    const root = agent.isLead
      ? agent
      : teamLead ||
        team.find((row) => row.id === agent.rootId && row.isLead) ||
        agent;
    const stored = useAgentDetail(agent);
    const rootDetail = useAgentDetail(root);
    const main = settingsFor(agent, false, receipt || stored || {});
    const defaults = settingsFor(root, true, workerDetail || rootDetail || {});
    const reviewer = reviewDetail || rootDetail;
    const catalog = useWorkerModels(agent.accountKey || "default", true);
    const workers = useWorkerModels(root.accountKey || "default", true, true);
    const summary = (
      model: string | null | undefined,
      effort: string | null | undefined,
      worker = false,
      review = false,
    ) => {
      const rows = review
        ? workers.models.filter((row) => row.provider !== ProviderKind.Claude)
        : worker
          ? workers.models
          : catalog.models;
      const info = modelForValue(rows, model || "default");
      const display = modelForValue(rows, info?.resolvedModel || model) || info;
      return {
        name: modelDisplayName(display, model || "Default"),
        provider: review
          ? ProviderKind.Codex
          : ((!worker ? agent.provider : undefined) ??
            info?.provider ??
            modelProvider(model ?? "") ??
            ""),
        effort: (effort || info?.defaultReasoningEffort || "").replace(
          /^./,
          (letter) => letter.toUpperCase(),
        ),
      };
    };
    const lead = summary(main.model, main.effort);
    const worker = summary(
      defaults.model ?? main.model,
      defaults.effort ?? main.effort,
      true,
    );
    const review =
      reviewer?.reviewDefaults?.model &&
      reviewer?.reviewDefaults.model !== INHERITED_MODEL
        ? summary(
            reviewer?.reviewDefaults.model,
            reviewer?.reviewDefaults.effort ?? main.effort,
            true,
            true,
          )
        : null;
    const entries = [
      {
        role: "lead",
        label: agent.isLead ? "Lead" : "Agent",
        value: lead,
        fast: main.fast_mode,
      },
      ...(root.isLead
        ? [
            // With subagents off only the lead and its reviews work.
            ...(subagentsOff(rootDetail || {})
              ? []
              : [
                  {
                    role: "worker",
                    label: "Workers",
                    value: worker,
                    fast: defaults.fast_mode,
                  },
                ]),
            {
              role: "review",
              label: "Review",
              value: review,
              fast: reviewer?.reviewDefaults?.fastMode ?? main.fast_mode,
            },
          ]
        : []),
    ];
    const full =
      "Execution settings: " +
      entries
        .map(
          ({ label, value, fast }) =>
            (value
              ? `${label} ${value.name} ${value.effort} (${value.provider})`.trim()
              : `${label}: Select a review model`) + (fast ? " Fast" : ""),
        )
        .join("; ");
    return (
      <ActionButton
        className={["execution-menu execution-summary-trigger", className]
          .filter(Boolean)
          .join(" ")}
        aria-label={full}
        title={full}
        aria-haspopup="dialog"
        onClick={onOpen}
        {...buttonProps}
      >
        <span className="execution-summary-segments" aria-hidden="true">
          {entries.map(({ role, label, value, fast }) => (
            <span
              className="execution-summary-segment"
              data-role={role}
              key={role}
            >
              <span className="execution-role">{label}</span>
              {value ? (
                <span className="execution-model">{value.name}</span>
              ) : (
                <span className="execution-inherited-label">
                  Select a review model
                </span>
              )}
              {fast && <span className="execution-effort">Fast</span>}
            </span>
          ))}
        </span>
      </ActionButton>
    );
  },
);
