import { SettingsNumber } from "../SettingsNumber/SettingsNumber.tsx";
import { Switch } from "@mantine/core";
import type { GetResult } from "../../../api.ts";
import {
  SHELL_QUIET_MIN_MS,
  SHELL_POLICY_DEFAULTS,
  MAX_SAFE_INTEGER,
  SHELL_PRESSURE_IDLE_MINUTES,
} from "../../../generated/sharedValues.ts";
import { SettingsRow } from "../../ui/SettingsRow/SettingsRow.tsx";

type Configuration = GetResult<"/api/orchestration/settings">;

type ShellPolicy = NonNullable<Configuration["settings"]["shellPolicy"]>;

export function BackgroundShellSettings({
  value = SHELL_POLICY_DEFAULTS,
  onChange,
  disabled,
}: {
  value?: ShellPolicy | undefined;
  onChange: (value: ShellPolicy) => void;
  disabled: boolean;
}) {
  return (
    <>
      <SettingsNumber
        label="Background shell check-in (milliseconds)"
        help={`Check once after a shell has no new output. Use 0 to disable, or at least ${SHELL_QUIET_MIN_MS} milliseconds.`}

        aria-label="Background shell quiet threshold in milliseconds"
        value={value.quietThresholdMs}
        min={0}
        max={MAX_SAFE_INTEGER}
        disabled={disabled}
        error={
          value.quietThresholdMs > 0 &&
          value.quietThresholdMs < SHELL_QUIET_MIN_MS
            ? `Use 0 or at least ${SHELL_QUIET_MIN_MS} milliseconds.`
            : undefined
        }
        onChange={(quietThresholdMs) =>
          onChange({ ...value, quietThresholdMs })
        }
      />
      <SettingsRow
        label="Stop idle shells under critical memory pressure"
        help={`After ${SHELL_PRESSURE_IDLE_MINUTES} idle minutes, stop background shells when no turn or delegate is active. Studio records each stop and notifies the agent.`}
      >
        <Switch
          aria-label="Stop idle shells under critical memory pressure"
          checked={value.pressureCleanup}
          disabled={disabled}
          onChange={(event) =>
            onChange({ ...value, pressureCleanup: event.currentTarget.checked })
          }
        />
      </SettingsRow>
    </>
  );
}
