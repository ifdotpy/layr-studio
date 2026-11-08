import { useSyncExternalStore } from "react";
import { useServerApi } from "./components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { PendingEntry } from "./pendingCommands.ts";

/** The pending commands of this server, in the order they began. */
export function usePendingCommands(): readonly PendingEntry[] {
  const { pending } = useServerApi();
  return useSyncExternalStore(pending.subscribe, pending.list, pending.list);
}
