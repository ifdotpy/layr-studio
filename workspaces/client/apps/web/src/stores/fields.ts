import { action } from "mobx";
import type { ChangeEvent } from "react";

/** Binds a string field of an observable model to a text input, textarea or native select, without hooks. */
export function textField<K extends string>(
  model: Record<K, string>,
  key: K,
): {
  value: string;
  onChange: (
    event: ChangeEvent<
      HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement
    >,
  ) => void;
} {
  return {
    value: model[key],
    onChange: action((event) => {
      model[key] = event.currentTarget.value;
    }),
  };
}
