import {
  useServerNet,
  useServerStorageEventKey,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useEffect, useMemo, useState } from "react";
import { onResume } from "../../../sync/resume.ts";
import type { QueueOptions } from "../../../servers/net/queue.ts";

export function useMessageQueue(p: QueueOptions) {
  const net = useServerNet();
  const serverStorageEventKey = useServerStorageEventKey();
  const [locks] = useState(() => new Set<string>());
  const queue = useMemo(
    () => net.queue(p, locks),
    [p.scope, p.id, p.enabled, p.workspaceId, net, locks],
  );
  useEffect(() => {
    queue.active = true;
    const changed = (event: StorageEvent) => {
      if (
        serverStorageEventKey(event) === queue.key ||
        serverStorageEventKey(event) === null
      )
        queue.syncPending();
    };
    window.addEventListener("storage", changed);
    const stopResume = onResume(queue.syncPending);
    const stopCommands = queue.subscribeCommands(queue.syncCommands);
    queue.syncPending();
    queue.syncCommands();
    return () => {
      queue.active = false;
      window.removeEventListener("storage", changed);
      stopResume();
      stopCommands();
    };
  }, [queue, serverStorageEventKey]);
  return queue;
}
