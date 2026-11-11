import {
  NativeSelect,
  NumberInput,
  Textarea,
  TextInput,
  type NativeSelectProps,
  type TextInputProps,
} from "@mantine/core";
import type { Json } from "../../../types.ts";

type Field = Pick<
  TextInputProps,
  "label" | "description" | "required" | "type"
> & {
  data?: NonNullable<NativeSelectProps["data"]>;
  minRows?: number;
  autosize?: boolean;
  min?: number;
  max?: number;
  allowDecimal?: boolean;
  allowNegative?: boolean;
  fallback?: string;
};
export type WorkspaceFieldsSpec = Record<string, Field | false>;

/** Bind the same durable form controls for profile and rule drafts. */
export function WorkspaceFields({
  fields,
  draft,
  change,
}: {
  fields: WorkspaceFieldsSpec;
  draft: Json;
  change: (key: string, value: string | number) => void;
}) {
  return Object.entries(fields).map(([key, field]) => {
    if (!field) return null;
    const { data, minRows, autosize, fallback, type, ...props } = field;
    const raw = draft[key];
    const value =
      (props.min !== undefined && typeof raw === "number") ||
      typeof raw === "string"
        ? raw
        : (fallback ?? "");
    const input = {
      ...props,
      value: String(value),
      onChange: (e: { target: { value: string } }) =>
        change(key, e.target.value),
    };
    if (data) return <NativeSelect key={key} {...input} data={data} />;
    if (props.min !== undefined)
      return (
        <NumberInput
          key={key}
          {...props}
          value={value}
          onChange={(value) => change(key, value)}
        />
      );
    if (minRows)
      return (
        <Textarea
          key={key}
          {...input}
          minRows={minRows}
          autosize={autosize ?? false}
        />
      );
    return <TextInput key={key} {...input} {...(type ? { type } : {})} />;
  });
}
