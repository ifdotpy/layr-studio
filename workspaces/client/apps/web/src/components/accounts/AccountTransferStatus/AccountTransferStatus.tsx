import { Button, Modal } from "@mantine/core";
import { layerZIndex } from "../../../theme.ts";
import { ArrowRightLeft } from "lucide-react";
import type { Agent } from "../../../types.ts";

export function AccountTransferStatus({
  transfer,
  targetLabel,
  pending,
  opened = true,
  onClose = () => {},
  error,
  onAction,
}: {
  transfer?: NonNullable<Agent["accountTransferProgress"]> | undefined;
  targetLabel?: string;
  pending?: boolean;
  opened?: boolean;
  onClose?: () => void;
  error?: string;
  onAction: (action: "retry" | "cancel") => void;
}) {
  const active = transfer && "actions" in transfer ? transfer : undefined;
  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title="Account transfer"
      centered
      zIndex={layerZIndex.modalAbovePopover}
    >
      {error && <p role="alert">{error}</p>}
      {transfer && transfer.kind !== "none" && (
        <div className="account-menu-note" role="status">
          <ArrowRightLeft size={14} />
          <div>
            {targetLabel && <div>Account: {targetLabel}</div>}
            <div>Transfer {transfer.kind}.</div>
            {"reason" in transfer && transfer.reason && (
              <small>{transfer.reason}</small>
            )}
            <div className="account-transfer-actions">
              {active?.actions.includes("retry") && (
                <Button
                  size="compact-xs"
                  variant="subtle"
                  disabled={!!pending}
                  onClick={() => onAction("retry")}
                >
                  Retry
                </Button>
              )}
              {active?.actions.includes("cancel") && (
                <Button
                  size="compact-xs"
                  variant="subtle"
                  disabled={!!pending}
                  onClick={() => onAction("cancel")}
                >
                  Cancel remaining
                </Button>
              )}
            </div>
          </div>
        </div>
      )}
    </Modal>
  );
}
