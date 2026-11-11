import { action, observable } from "mobx";
import type { PreferenceScope } from "./uiPreferenceMerge.ts";

export type PreferenceWriteStatus =
  | { state: "saved" }
  | { state: "saving" }
  | { state: "error"; error: string };
const saved: PreferenceWriteStatus = { state: "saved" };
const statuses = new WeakMap<
  Storage,
  Record<PreferenceScope, PreferenceWriteStatus>
>();
function forStorage(storage: Storage) {
  const status =
    statuses.get(storage) ?? observable({ user: saved, server: saved });
  statuses.set(storage, status);
  return status;
}
const retries = new WeakMap<Storage, () => void>();
export const readPreferenceWriteStatus = (
  storage: Storage,
  scope: PreferenceScope,
) => forStorage(storage)[scope];
export const setPreferenceWriteStatus = action(
  (storage: Storage, scope: PreferenceScope, status: PreferenceWriteStatus) => {
    forStorage(storage)[scope] = status;
  },
);
export function registerPreferenceRetry(storage: Storage, retry: () => void) {
  retries.set(storage, retry);
  return () => {
    if (retries.get(storage) === retry) retries.delete(storage);
  };
}
export function retryPreferenceWrites(storage: Storage) {
  retries.get(storage)?.();
}
