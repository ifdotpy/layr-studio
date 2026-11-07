import { autorun, observable, runInAction } from "mobx";
import { useEffect, useMemo } from "react";
import { useServerStore } from "../servers/ServerServicesContext";
import type { MessageReceipt } from "../sync/messageReceipts";

/** Confirmed receipts survive an older retained pending input until it leaves
 * the requested window. This overlay belongs to this server and conversation. */
export function useMessageReceipts(
  room: string | null,
  scope: string,
  workspaceId: string | undefined,
  ids: string[],
) {
  const store = useServerStore();
  const receipts = useMemo(
    () => observable.map<string, MessageReceipt>(undefined, { deep: false }),
    [store, scope, room],
  );
  const requestedKey = JSON.stringify(ids);
  useEffect(() => {
    if (!room || !workspaceId) return;
    const keep = new Set(ids);
    return autorun(() => {
      const current = store.state().current;
      if (current.status !== "live" && current.status !== "stale") return;
      const selected = ids.flatMap((id) => {
        const receipt = current.data.messageReceipts.get(id);
        return receipt?.agent === room ? [receipt] : [];
      });
      runInAction(() => {
        for (const receipt of selected) receipts.set(receipt.id, receipt);
        for (const id of receipts.keys())
          if (!keep.has(id)) receipts.delete(id);
      });
    });
  }, [store, receipts, room, workspaceId, requestedKey]);
  return receipts;
}
