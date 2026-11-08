import { useEffect } from "react";

declare global {
  interface Window {
    __studioMessageRenderProbe?: (leaf: string, id?: string) => void;
  }
}
/** Count committed real leaves without creating another state owner. */
export function useMessageRenderProbe(leaf: string, id?: string) {
  useEffect(() => {
    window.__studioMessageRenderProbe?.(leaf, id);
  });
}
