import { UserMessage } from "../UserMessage/UserMessage.tsx";
import { resourceData } from "../../../../store/runtime.ts";
import {
  useServerStorage,
  useServerStore,
} from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useMemo } from "react";
import { observer } from "mobx-react-lite";

import { complaintResponseStore } from "../support/complaintReplyRequest.ts";

export default observer(function UserMessages({
  target = "user",
  hideEmpty = false,
  focusId,
  onlyComplaintId,
  focusRequestId,
}: {
  target?: "user" | "lead";
  hideEmpty?: boolean;
  focusId?: string | undefined;
  onlyComplaintId?: string;
  focusRequestId?: string | undefined;
}) {
  const storage = useServerStorage();
  const store = useServerStore();
  const state = resourceData(store.state().current);
  const stateDir = state?.workspaces.values().next().value?.stateDir || "";
  const pending = useMemo(() => {
    try {
      return new Map(
        Object.entries(complaintResponseStore(storage, stateDir).read()),
      );
    } catch {
      return new Error(
        "The saved response request cannot be read. Resolve it before sending another response.",
      );
    }
  }, [stateDir, storage]);
  const selected = onlyComplaintId
    ? state?.complaints.get(onlyComplaintId)
    : undefined;
  const records = (
    onlyComplaintId
      ? selected
        ? [selected]
        : []
      : [...(state?.complaints.values() || [])]
  ).filter((message) => message.recipient === target);
  return (
    <section className="user-message-list">
      {pending instanceof Error ? (
        <p role="alert">{pending.message}</p>
      ) : (
        records.map((message) => (
          <UserMessage
            key={JSON.stringify([stateDir, message.id])}
            message={message}
            scope={stateDir}
            focused={message.id === focusId}
            focusRequestId={focusRequestId}
            requests={pending}
          />
        ))
      )}
      {!hideEmpty && !records.length && (
        <p className="notice">No messages yet.</p>
      )}
    </section>
  );
});
