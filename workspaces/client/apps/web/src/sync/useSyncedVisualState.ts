import {
  useServerPreferences,
  useServerStorageEventKey,
} from "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useEffect, useState } from "react";
import { preferenceEvent } from "./uiPreferenceMerge.ts";
/** Local cache supplies the first render; entity delivery updates later renders. */
export function useSyncedVisualState<T>(key: string, fallback: T) {
  const { saved } = useServerPreferences();
  const storageEventKey = useServerStorageEventKey();

  const [value, setValue] = useState<T>(() => saved(key, fallback));
  useEffect(() => {
    const update = (event?: Event) => {
      if (event instanceof CustomEvent && event.detail !== key) return;
      if (
        event instanceof StorageEvent &&
        event.key !== null &&
        storageEventKey(event) !== key
      )
        return;
      setValue(saved(key, fallback));
    };
    update();
    window.addEventListener(preferenceEvent, update);
    window.addEventListener("storage", update);
    return () => {
      window.removeEventListener(preferenceEvent, update);
      window.removeEventListener("storage", update);
    };
  }, [key, saved, storageEventKey]);
  return [value, setValue] as const;
}
