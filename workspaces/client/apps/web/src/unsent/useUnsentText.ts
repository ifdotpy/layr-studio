import { useCallback, useEffect, useRef, useState } from "react";
import { useServerServices } from "../servers/ServerServicesContext.tsx";
import { serverViewId } from "../servers/environment.ts";
import { unsentStore } from "./store.ts";

/** Keep a text edit on this device until its caller clears it. */
export function useUnsentText(feature: string, id: string) {
  const server = useServerServices()?.owner.id ?? serverViewId ?? "local";
  const identity = JSON.stringify([server, feature, id]);
  const [value, setValue] = useState<string | null>(null);
  const current = useRef({ identity, changed: false });
  if (current.current.identity !== identity)
    current.current = { identity, changed: false };
  useEffect(() => {
    let active = true;
    const owner = current.current;
    setValue(null);
    void unsentStore(server, feature)
      .loadAll()
      .then((rows) => {
        const saved = rows.find(([key]) => key === id)?.[1];
        if (active && !owner.changed && typeof saved === "string")
          setValue(saved);
      })
      .catch((error: unknown) =>
        console.error("Could not restore unsent input", error),
      );
    return () => {
      active = false;
    };
  }, [identity]);
  const clear = useCallback(() => {
    current.current.changed = true;
    return unsentStore(server, feature).delete(id);
  }, [identity]);
  const update = useCallback(
    (next: string | null) => {
      current.current.changed = true;
      setValue(next);
      const operation =
        next === null ? clear() : unsentStore(server, feature).put(id, next);
      void operation.catch((error: unknown) =>
        console.error("Could not save unsent input", error),
      );
    },
    [identity],
  );
  return [value, update, clear] as const;
}
