import type { OrchestrationSettingSpec } from "../support/OrchestrationSettings.tsx";
import { NumberInput } from "@mantine/core";
import { SettingsRow } from "../../ui/SettingsRow/SettingsRow.tsx";

export function OrchestrationNumberSetting({
  spec,
  value,
  disabled,
  onChange,
}: {
  spec: OrchestrationSettingSpec;
  value: number;
  disabled: boolean;
  onChange: (value: number) => void;
}) {
  return (
    <SettingsRow label={spec.label} help={spec.help}>
      <NumberInput
        aria-label={spec.ariaLabel}
        title={spec.tooltip}
        min={spec.min}
        max={spec.max}
        allowDecimal={false}
        allowNegative={false}
        clampBehavior="strict"
        value={value}
        disabled={disabled}
        onChange={(next) => {
          if (typeof next === "number") onChange(next);
        }}
      />
    </SettingsRow>
  );
}
