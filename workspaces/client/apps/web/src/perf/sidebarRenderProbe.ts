/** Optional field-isolation probe for the real sidebar component tree. */
export function sidebarRender(component: string, id?: string) {
  if (typeof window !== "undefined")
    window.__studioSidebarRenderProbe?.(component, id);
}
declare global {
  interface Window {
    __studioSidebarRenderProbe?: (component: string, id?: string) => void;
  }
}
