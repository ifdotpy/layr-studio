declare global {
  interface Window {
    __studioMessageRenderProbe?: (leaf: string, id?: string) => void;
  }
}
/** Count real leaf renders without creating another state owner. */
export function useMessageRenderProbe(leaf: string, id?: string) {
  if (typeof window !== "undefined")
    window.__studioMessageRenderProbe?.(leaf, id);
}
