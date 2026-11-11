import { NumberInput, type NumberInputProps } from "@mantine/core";
import { SettingsRow } from "../../ui/SettingsRow/SettingsRow.tsx";

/** Shared whole-number settings field; each caller owns its bounds and unit. */
export function SettingsNumber({
  label,
  help,
  onChange,
  ...input
}: Omit<NumberInputProps, "label" | "description" | "onChange"> & {
  label: string;
  help: string;
  onChange: (value: number) => void;
}) {
  return (
    <SettingsRow label={label} help={help}>
      <NumberInput
        {...input}
        allowDecimal={false}
        allowNegative={false}
        onChange={(value) => {
          if (typeof value === "number") onChange(value);
        }}
      />
    </SettingsRow>
  );
}
