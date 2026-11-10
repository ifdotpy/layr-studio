import { observer } from "mobx-react-lite";
import { useState } from "react";
import { SegmentedControl, NumberInput, Button } from "@mantine/core";
import {
  useServerStore,
  useServerStorage,
  useServerApi,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { Agent } from "../../../types.ts";
import {
  DEFAULT_SUBAGENT_CONCURRENCY as DEFAULT_CONCURRENCY,
  MIN_SUBAGENT_CONCURRENCY as MIN_CONCURRENCY,
  MAX_SUBAGENT_CONCURRENCY as MAX_CONCURRENCY,
} from "../../../generated/apiSchema.ts";
import { ConcurrencyStore } from "./ConcurrencyStore.ts";
export { subagentsOff, decodeStoredConcurrency } from "./ConcurrencyStore.ts";
const CONCURRENCY_HELP = "0 = lead only; extra work queues.";
const UNSUPPORTED_HELP =
  "Update the backend to enable numeric subagent parallelism. Saved changes are retained until then.";
type Props = {
  compact?: boolean;
  lead: Agent;
  stateDir: string;
  workspaceId: string;
};
export default function SubagentConcurrencyControl(props: Props) {
  const scope = JSON.stringify([
    props.stateDir,
    props.workspaceId,
    props.lead.id,
  ]);
  return <ScopedConcurrencyControl key={scope} {...props} scope={scope} />;
}
const ScopedConcurrencyControl = observer(function ScopedConcurrencyControl({
  lead,
  compact = false,
  workspaceId,
  scope,
}: Props & { scope: string }) {
  const owner = useServerStore(),
    storage = useServerStorage(),
    api = useServerApi();
  const [control] = useState(
    () => new ConcurrencyStore(lead, owner, api, storage, scope, workspaceId),
  );
  control.lead = lead;
  const {
    confirmed,
    supported,
    draft,
    mode,
    pending,
    saving,
    error,
    storageError,
    disabled,
    submit,
  } = control;
  return (
    <form
      className={`agent-mode-control${compact ? " agent-mode-compact" : ""}`}
      data-agent-mode={mode ?? "unknown"}
      {...(supported && confirmed
        ? {
            "data-agent-mode-revision": confirmed.agentModeRevision,
            "data-concurrency": confirmed.concurrency,
          }
        : {})}
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      {compact && (
        <SegmentedControl
          aria-label="Parallel"
          value={mode === "single" ? "off" : "on"}
          data={[
            { value: "off", label: "Off" },
            { value: "on", label: "On" },
          ]}
          disabled={disabled}
          onChange={(value) => {
            const target = value === "off" ? 0 : DEFAULT_CONCURRENCY;
            control.setDraft(target);
            void submit(target);
          }}
        />
      )}
      <div className="agent-mode-fields">
        <NumberInput
          className="agent-mode-field"
          label="Subagent parallelism"
          description={supported ? CONCURRENCY_HELP : UNSUPPORTED_HELP}
          min={MIN_CONCURRENCY}
          max={MAX_CONCURRENCY}
          step={1}
          allowDecimal={false}
          value={supported ? draft : ""}
          disabled={disabled}
          onBlur={(event) => {
            const nextMode = (event.relatedTarget as HTMLInputElement | null)
              ?.value;
            if (compact && nextMode !== (mode === "single" ? "on" : "off"))
              void submit();
          }}
          onChange={(value) => {
            control.setDraft(value);
          }}
        />
        {!compact && draft !== confirmed?.concurrency && (
          <Button
            type="submit"
            className="agent-mode-apply"
            disabled={disabled}
          >
            Apply
          </Button>
        )}
      </div>
      <span id={`subagent-concurrency-help-${lead.id}`} className="sr-only">
        {CONCURRENCY_HELP}
      </span>
      <span
        id={`subagent-concurrency-status-${lead.id}`}
        className={supported ? "sr-only" : "agent-mode-note"}
        role={supported ? undefined : "status"}
      >
        {supported
          ? ""
          : "Numeric subagent parallelism is unavailable because this backend does not provide the concurrency setting. Update the backend to enable it."}
      </span>
      {saving ? (
        <span role="status" className="agent-mode-note">
          Applying…
        </span>
      ) : pending && supported ? (
        <Button
          type="button"
          className="agent-mode-retry"
          disabled={!!storageError}
          onClick={() => void submit()}
        >
          {pending.agent_mode ? "Retry mode change" : "Retry limit change"}
        </Button>
      ) : pending ? (
        <span className="agent-mode-note" role="status">
          Saved change retained. Update the backend to retry it.
        </span>
      ) : !compact && confirmed?.concurrency === 0 ? (
        <span className="agent-mode-note">No new worker turns will start.</span>
      ) : null}
      {error && (
        <span className="agent-mode-error" role="alert">
          {error}
        </span>
      )}
      {storageError && supported && (
        <Button
          type="button"
          className="agent-mode-retry"
          onClick={control.discard}
        >
          Discard unreadable concurrency request
        </Button>
      )}
    </form>
  );
});
