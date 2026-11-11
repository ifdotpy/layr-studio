import { WorkspaceResourceState as ResourceState } from "../WorkspaceResourceState/WorkspaceResourceState.tsx";
import {
  date,
  type PanelContext,
} from "../WorkspaceSection/WorkspaceSection.tsx";
import { Button, Modal } from "@mantine/core";
import { Plus } from "lucide-react";
import { action } from "mobx";
import { observer } from "mobx-react-lite";
import ErrorDescription from "../ErrorDescription/ErrorDescription.tsx";
import { WorkspaceFields } from "../WorkspaceFields/WorkspaceFields.tsx";
import type { RuleRecord } from "../../../app/shell/WorkspaceRules.ts";
import {
  lowWorkerDefaults,
  ruleDraftFromRecord,
} from "../../../app/shell/WorkspaceRules.ts";
import {
  section,
  WorkspaceSection as Empty,
} from "../WorkspaceSection/WorkspaceSection.tsx";
import { WorkspaceStatus as Status } from "../WorkspaceStatus/WorkspaceStatus.tsx";

export const WorkspaceRules = section(
  observer(function WorkspaceRules(c: PanelContext<RuleRecord>) {
    const state = c.model.ruleState;
    const ui = c.panel;
    const { busy, remove } = ui;
    const draft = c.model.ruleDraft;
    const setDraft = c.model.setRuleDraft;
    const rules = c.model.rules;
    return (
      <>
        <ResourceState state={state} />
        <div className="workspace-toolbar">
          <span className="workspace-muted">
            The model does not run while a rule waits.
          </span>
          <Button
            size="xs"
            variant="filled"
            leftSection={<Plus size={14} />}
            onClick={c.model.newRule}
          >
            New rule
          </Button>
        </div>
        {rules.map(({ rule, paused, toggleAction, toggleLabel }) => (
          <div className="workspace-row" key={rule.id}>
            <div className="workspace-row-head">
              <strong>{rule.name}</strong>
              <Status value={rule.status || (paused ? "paused" : "active")} />
            </div>
            <small>
              {rule.kind === "low_workers"
                ? `Fewer than ${rule.minimumWorkers ?? lowWorkerDefaults.minimumWorkers} active subagents for more than ${rule.durationMinutes ?? lowWorkerDefaults.durationMinutes} minutes`
                : rule.kind}
              {rule.kind !== "low_workers" && rule.intervalSeconds
                ? ` · every ${rule.intervalSeconds}s`
                : ""}
              {rule.kind !== "low_workers" && rule.nextAt
                ? ` · next ${date(rule.nextAt)}`
                : ""}
            </small>
            {rule.path && (
              <p>
                <code>{rule.path}</code>
              </p>
            )}
            {rule.error && (
              <ErrorDescription
                className="workspace-error"
                value={rule.error}
              />
            )}
            <p className="workspace-clamp">{rule.text}</p>
            <p className="workspace-muted">
              {rule.checks || 0} checks · {rule.wakes || 0} agent turns
              {rule.lastExitCode != null
                ? ` · last exit ${rule.lastExitCode}`
                : ""}
            </p>
            {rule.lastOutput && (
              <details className="workspace-tool">
                <summary>Last check output</summary>
                <pre className="workspace-code">{rule.lastOutput}</pre>
              </details>
            )}
            <div className="workspace-actions">
              <Button
                size="compact-xs"
                variant="subtle"
                onClick={() => setDraft(ruleDraftFromRecord(rule))}
              >
                Edit
              </Button>
              <Button
                size="compact-xs"
                variant="light"
                disabled={busy}
                onClick={() => void ui.ruleAction(c, rule, toggleAction)}
              >
                {toggleLabel}
              </Button>
              <Button
                size="compact-xs"
                variant="subtle"
                color="red"
                onClick={action(() => (ui.remove = rule))}
              >
                Delete
              </Button>
            </div>
          </div>
        ))}
        {!rules.length && state.data !== null && (
          <Empty>No rules for this agent.</Empty>
        )}
        <Modal
          opened={!!draft}
          onClose={() => {
            if (!busy) setDraft(null);
          }}
          title={draft?.id && !draft.isNew ? "Edit rule" : "New rule"}
          size="lg"
        >
          {draft && (
            <form
              className="workspace-form"
              onSubmit={(e) => {
                e.preventDefault();
                void ui.saveRule(c);
              }}
            >
              <WorkspaceFields
                draft={{ ...draft }}
                change={c.model.changeRule}
                fields={c.model.ruleFields}
              />
              {draft.kind === "low_workers" && (
                <p className="workspace-muted">
                  Alerts the main agent once when fewer subagents than the
                  minimum are active.
                </p>
              )}
              <Button variant="filled" type="submit" loading={busy}>
                Save rule
              </Button>
            </form>
          )}
        </Modal>
        <Modal
          opened={!!remove}
          onClose={action(() => {
            if (!ui.busy) ui.remove = null;
          })}
          title="Delete rule"
        >
          <p>Delete “{remove?.name}”?</p>
          <Button
            variant="filled"
            color="red"
            loading={busy}
            onClick={() => remove && void ui.ruleAction(c, remove, "delete")}
          >
            Delete rule
          </Button>
        </Modal>
      </>
    );
  }),
);
