import { useServerPreferences } from "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
/** Local cache supplies the first render; observable delivery updates later renders. */
export function useSyncedVisualState<T>(key: string, fallback: T) {
  return [useServerPreferences().saved(key, fallback)] as const;
}
