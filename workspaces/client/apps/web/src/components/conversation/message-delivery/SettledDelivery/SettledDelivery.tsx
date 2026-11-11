import type { ReactNode } from "react";
import { Owned } from "../../../ui/Owned/Owned.tsx";
import { reached } from "../../../../clock.ts";

/** How long a transient delivery step must last before it shows, in ms. */
export const DELIVERY_SETTLE_MS = 1000;

/** Outcomes show immediately; transient steps share the observed clock. */
export default function SettledDelivery({
  transient,
  children,
}: {
  transient: boolean;
  children: ReactNode;
}) {
  return transient ? (
    <Owned create={() => Date.now() + DELIVERY_SETTLE_MS}>
      {(deadline) => (reached(deadline) ? children : null)}
    </Owned>
  ) : (
    children
  );
}
