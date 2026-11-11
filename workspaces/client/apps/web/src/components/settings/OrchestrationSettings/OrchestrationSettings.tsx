import type { OrchestrationSettingSpec } from "../support/OrchestrationSettings.tsx";
import { OrchestrationNumberSetting } from "../OrchestrationNumberSetting/OrchestrationNumberSetting.tsx";
import {
  IdleCompactionSettings,
  idleCompactionDefaults,
} from "../IdleCompactionSettings/IdleCompactionSettings.tsx";
import { BackgroundShellSettings } from "../BackgroundShellSettings/BackgroundShellSettings.tsx";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import {
  CHILD_RESULT_COALESCE_MAX_SECONDS,
  CHILD_RESULT_COALESCE_MIN_SECONDS,
  IDLE_SESSION_UNLOAD_MAX_MINUTES,
  IDLE_SESSION_UNLOAD_MIN_MINUTES,
} from "../../../generated/sharedValues.ts";

export const OrchestrationSettings = observer(function OrchestrationSettings({
  active,
}: {
  active: boolean;
}) {
  const model = useServerAreas().settingsModels.orchestration;
  if (!active) return null;
  const { configuration, error, busy, setConfiguration } = model;
  return (
    <section className="settings-group" aria-label="Agent orchestration">
      <h2>Agent orchestration</h2>
      {configuration && (
        <>
          {SPECS.map((spec) => (
            <OrchestrationNumberSetting
              key={spec.field}
              spec={spec}
              value={configuration.settings[spec.field]}
              disabled={busy}
              onChange={(value) => model.patch({ [spec.field]: value })}
            />
          ))}
          <BackgroundShellSettings
            configuration={configuration}
            setConfiguration={setConfiguration}
            disabled={busy}
          />
          <IdleCompactionSettings
            value={
              configuration.settings.idleCompaction ?? idleCompactionDefaults
            }
            disabled={busy}
            onChange={(value) =>
              setConfiguration({
                ...configuration,
                settings: { ...configuration.settings, idleCompaction: value },
              })
            }
          />
          <Button mt="md" loading={busy} onClick={model.save}>
            Save orchestration settings
          </Button>
        </>
      )}
      {error && <p role="alert">{error}</p>}
    </section>
  );
});

export const childResultHoldSpec: OrchestrationSettingSpec = {
  field: "childResultCoalesceSeconds",
  label: "Subagent result hold (seconds)",
  help: "Deliver results that finish close together in one lead turn.",
  ariaLabel: "Subagent result hold in seconds",
  tooltip:
    "When a lead has several subagents running, a finished subagent's result is held " +
    "for up to this many seconds so results that arrive close together are delivered " +
    "to the lead in one turn. The lead is woken immediately when all its subagents " +
    "have finished or when any other message arrives. 0 delivers every result immediately.",
  min: CHILD_RESULT_COALESCE_MIN_SECONDS,
  max: CHILD_RESULT_COALESCE_MAX_SECONDS,
};

export const idleSessionUnloadSpec: OrchestrationSettingSpec = {
  field: "idleSessionUnloadMinutes",
  label: "Unload idle sessions after (minutes)",
  help: "Free the memory of leads and chats that have nothing to do.",
  ariaLabel: "Unload idle sessions after minutes",
  tooltip:
    "A lead's or ordinary chat's Codex thread or Claude session without a running " +
    "turn, pending input, or active monitor is unloaded after this many minutes " +
    "and resumed automatically on the next message. An account's provider process " +
    "stops after this many minutes with no loaded thread. Workers spawned by a " +
    "lead unload as soon as their turn ends. 0 never unloads.",
  min: IDLE_SESSION_UNLOAD_MIN_MINUTES,
  max: IDLE_SESSION_UNLOAD_MAX_MINUTES,
};

const SPECS = [childResultHoldSpec, idleSessionUnloadSpec];
