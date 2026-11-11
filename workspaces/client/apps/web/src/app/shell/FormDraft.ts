import { makeAutoObservable } from "mobx";
import { isRecord } from "../../components/shell/McpInventory/McpInventory.tsx";
import type { Json, JsonValue } from "../../types.ts";
import type { UnsentText } from "../../unsent/useUnsentText.ts";

function isJsonValue(value: unknown): value is JsonValue {
  return (
    value === null ||
    ["string", "boolean", "number"].includes(typeof value) ||
    (typeof value === "object" && Object.values(value).every(isJsonValue))
  );
}
function isDraft(value: unknown): value is Json {
  return isRecord(value) && Object.values(value).every(isJsonValue);
}
function readDraft(raw: string | null): Json | null {
  if (!raw) return null;
  try {
    const value: unknown = JSON.parse(raw);
    return isDraft(value) ? value : null;
  } catch {
    return null;
  }
}

/** Durable profile and rule forms contain no credential fields. */
export class FormDraft {
  readonly text;
  readonly key;
  constructor(text: UnsentText, key: string) {
    this.text = text;
    this.key = key;
    makeAutoObservable(this, { text: false }, { autoBind: true });
  }
  get serialized() {
    return this.text.read("form", this.key);
  }
  get value() {
    return readDraft(this.serialized);
  }
  update(next: Json | null) {
    this.text.update(
      "form",
      this.key,
      next === null ? null : JSON.stringify(next),
    );
  }
  change(key: string, value: string | number) {
    const current = this.value;
    if (current) this.update({ ...current, [key]: value });
  }
  clear(expected?: string | null) {
    return this.text.clear("form", this.key, expected);
  }
}
