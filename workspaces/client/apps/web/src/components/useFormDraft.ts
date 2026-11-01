import { useRef, type SetStateAction } from "react";
import type { Json, JsonValue } from "../types";
import { useUnsentText } from "../unsent/useUnsentText";

function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "boolean" ||
    typeof value === "number"
  )
    return true;
  if (Array.isArray(value)) return value.every(isJsonValue);
  return typeof value === "object" && Object.values(value).every(isJsonValue);
}
function readDraft(raw: string | null): Json | null {
  if (!raw) return null;
  try {
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== "object" || Array.isArray(value))
      return null;
    const result: Json = {};
    for (const [key, entry] of Object.entries(value)) {
      if (!isJsonValue(entry)) return null;
      result[key] = entry;
    }
    return result;
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
