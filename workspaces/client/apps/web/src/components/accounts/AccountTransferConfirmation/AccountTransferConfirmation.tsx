import type { Account } from "../support/Accounts.tsx";
import { appendAppLog } from "../../../appLog.ts";
import { Button, Modal } from "@mantine/core";
import { layerZIndex } from "../../../theme.ts";
import { useEffect, useRef, useState } from "react";
import { errorText } from "../../../api.ts";

export function AccountTransferConfirmation({
  opened,
  onClose,
  target,
  sourceProvider,
  onConfirm,
}: {
  opened: boolean;
  onClose: () => void;
  target: Account | null;
  sourceProvider: string;
  onConfirm: () => Promise<void>;
}) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const lock = useRef(false);
  useEffect(() => {
    setError("");
  }, [target?.id, opened]);
  return (
    <Modal
      opened={opened}
      zIndex={layerZIndex.modalAbovePopover}
      onClose={() => {
        if (!pending) onClose();
      }}
      title="Transfer this chat"
      closeOnClickOutside={!pending}
      closeOnEscape={!pending}
      withCloseButton={!pending}
    >
      <p>
        Transfer this chat to <strong>{target?.email || target?.label}</strong>?
      </p>
      <p>
        Active turns move immediately and continue on the destination. Subagents
        keep their accounts.
      </p>
      {target && sourceProvider !== target.provider && (
        <p>The destination model uses the saved chat context.</p>
      )}
      <p>Applies to this chat only.</p>
      {error && <p role="alert">{error}</p>}
      <Button variant="default" disabled={pending} onClick={onClose}>
        Cancel
      </Button>
      <Button
        loading={pending}
        disabled={!target || pending}
        onClick={async () => {
          if (lock.current) return;
          lock.current = true;
          setPending(true);
          setError("");
          try {
            await onConfirm();
            onClose();
          } catch (failure) {
            appendAppLog({
              at: Date.now(),
              level: "error",
              title: "Account transfer failed",
              reason: errorText(failure),
            });
            setError(errorText(failure));
          } finally {
            lock.current = false;
            setPending(false);
          }
        }}
      >
        Transfer chat
      </Button>
    </Modal>
  );
}
