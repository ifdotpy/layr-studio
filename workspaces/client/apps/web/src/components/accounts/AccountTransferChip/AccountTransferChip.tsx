import { Tooltip } from "@mantine/core";
import { ArrowRightLeft } from "lucide-react";
import type { Agent } from "../../../types.ts";

export function AccountTransferChip({
  transfer,
}: {
  transfer?: NonNullable<Agent["accountTransferProgress"]> | undefined;
}) {
  if (transfer?.kind !== "blocked" && transfer?.kind !== "unreadable")
    return null;
  const label =
    transfer.kind === "unreadable"
      ? "Account move unreadable"
      : "Account move blocked";
  const details = transfer.reason;
  return (
    <Tooltip label={details} multiline w={320} withinPortal>
      <span
        className="account-transfer-chip"
        data-blocked=""
        role="status"
        aria-label={label + ". " + details}
      >
        <ArrowRightLeft size={12} />
        {label}
      </span>
    </Tooltip>
  );
}
