import { useEffect, useState, type ReactNode } from "react";

/** How long a transient delivery step must last before it shows, in ms. */
export const DELIVERY_SETTLE_MS = 1000;

/**
 * A normal send passes through Sending and Queued within a second and ends
 * at Sent. Only an outcome, or a step that lasts, is worth showing: the user
 * then sees one change instead of three.
 */
export default function SettledDelivery({
  transient,
  children,
}: {
  transient: boolean;
  children: ReactNode;
}) {
  const [settled, setSettled] = useState(!transient);
  useEffect(() => {
    if (!transient) {
      setSettled(true);
      return;
    }
    setSettled(false);
    const timer = setTimeout(() => setSettled(true), DELIVERY_SETTLE_MS);
    return () => clearTimeout(timer);
  }, [transient]);
  return settled ? children : null;
}
