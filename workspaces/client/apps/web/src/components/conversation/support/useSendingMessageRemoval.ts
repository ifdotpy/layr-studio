import { useMemo } from "react";
import {
  useServerApi,
  useServerOutbox,
  useServerStorage,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createSendingMessageRemoval } from "../../../app/conversation/delivery/removeSendingMessages.ts";

export function useSendingMessageRemoval() {
  const { get, command } = useServerApi();
  const { stopRemovedMessage } = useServerOutbox();
  const localStorage = useServerStorage();
  return useMemo(
    () =>
      createSendingMessageRemoval({
        get,
        command,
        stopRemovedMessage,
        localStorage,
      }),
    [get, command, stopRemovedMessage, localStorage],
  );
}
