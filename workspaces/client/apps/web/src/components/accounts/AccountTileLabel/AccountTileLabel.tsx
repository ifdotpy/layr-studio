import { remainingText } from "../support/AccountTiles.tsx";
import type { ReactNode } from "react";

/** The name and remaining limit inside an account tile. */
export function AccountTileLabel({
  name,
  remaining,
  nameClassName = "setup-account-name",
  children,
}: {
  name: string;
  remaining: number | null;
  nameClassName?: string;
  children?: ReactNode;
}) {
  return (
    <span className="setup-account-label">
      <span className={nameClassName}>
        {children} {name}
      </span>
      {remaining !== null && (
        <small className="account-remaining">{remainingText(remaining)}</small>
      )}
    </span>
  );
}
