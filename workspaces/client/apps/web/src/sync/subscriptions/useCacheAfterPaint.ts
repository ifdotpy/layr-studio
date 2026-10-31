import { useEffect } from "react";
import { useServerServices } from "../../servers/ServerServicesContext";
import type { ResourceRef } from "./contracts";

/** A second animation frame follows the paint of the committed view. */
export function useCacheAfterPaint(ref: ResourceRef | null, shown: unknown) {
  const services = useServerServices();
  useEffect(() => {
    if (
      !services ||
      !ref ||
      shown == null ||
      typeof requestAnimationFrame !== "function"
    )
      return;
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => {
        void services.sync.cacheSubscription(ref);
      });
    });
    return () => cancelAnimationFrame(frame);
  }, [services, ref, shown]);
}
