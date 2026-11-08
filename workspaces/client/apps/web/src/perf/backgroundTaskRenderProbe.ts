declare global {
  interface Window {
    __studioBackgroundTaskRenderProbe?: (leaf: string, id?: string) => void;
  }
}
export function probe(leaf: string, id?: string) {
  if (typeof window !== "undefined")
    window.__studioBackgroundTaskRenderProbe?.(leaf, id);
}
