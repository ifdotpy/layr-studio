import { isRecord } from "../McpInventory/McpInventory.tsx";
import { useRef, type SetStateAction } from "react";
import type { Json, JsonValue } from "../../../types.ts";
import { useUnsentText } from "../../../unsent/useUnsentText.ts";

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

// These profile and rule forms contain no credential fields.
export function useFormDraft(key: string) {
  const [raw, save, clear] = useUnsentText("form", key);
  const current = useRef<Json | null>(readDraft(raw));
  const lastRaw = useRef(raw);
  if (lastRaw.current !== raw) {
    lastRaw.current = raw;
    current.current = readDraft(raw);
  }
  const update = (change: SetStateAction<Json | null>) => {
    const next =
      typeof change === "function" ? change(current.current) : change;
    current.current = next;
    const text = next === null ? null : JSON.stringify(next);
    lastRaw.current = text;
    save(text);
  };
  return [current.current, update, clear] as const;
}
