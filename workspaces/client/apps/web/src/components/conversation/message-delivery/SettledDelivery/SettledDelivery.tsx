import { useState, type ReactNode } from "react";
import { observer } from "mobx-react-lite";
import { reached } from "../../../../clock.ts";

/** How long a transient delivery step must last before it shows, in ms. */
export const DELIVERY_SETTLE_MS = 1000;

const TransientDelivery = observer(function TransientDelivery({
  children,
}: {
  children: ReactNode;
}) {
  const [started] = useState(Date.now);
  return reached(started + DELIVERY_SETTLE_MS) ? children : null;
});

/** Outcomes show immediately; transient steps share the observed clock. */
export default function SettledDelivery({
  transient,
  children,
}: {
  transient: boolean;
  children: ReactNode;
}) {
  return transient ? (
    <TransientDelivery>{children}</TransientDelivery>
  ) : (
    children
  );
}
