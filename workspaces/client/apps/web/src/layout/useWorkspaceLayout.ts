import { layoutValues } from "./layout-contract.generated";
import { useCallback, useLayoutEffect, useState } from "react";
import {
  deriveWorkspaceLayout,
  type LayoutRect,
} from "./deriveWorkspaceLayout";

const emptyRect: LayoutRect = { x: 0, y: 0, width: 0, height: 0 };
/** Observe the common row. Its parent already removes navigation and safe areas. */
export function useWorkspaceLayout(options: {
  workspaceOpen?: boolean;
  teamPreferred?: boolean;
  preferredPanelWidthRem?: number;
}) {
  const [host, setHost] = useState<HTMLDivElement | null>(null);
  const [geometry, setGeometry] = useState<{
    host: LayoutRect;
    viewport: LayoutRect;
    rootFontPx: number;
    shortTouchLandscape: boolean;
  }>({
    host: emptyRect,
    viewport: emptyRect,
    rootFontPx: layoutValues.viewport.rootFontDefault,
    shortTouchLandscape: false,
  });
  const rowRef = useCallback(
    (node: HTMLDivElement | null) => setHost(node),
    [],
  );
  useLayoutEffect(() => {
    if (!host) return;
    const document = host.ownerDocument;
    const view = document.defaultView!;
    const root = document.documentElement;
    // Root style writes come every frame during drags (custom properties);
    // only these inputs change the root font size or the landscape flag.
    const inputs = () =>
      `${root.className}|${root.style.fontSize}|${root.getAttribute("data-viewport-short-landscape")}`;
    let measured: string | undefined;
    let rootFontPx = Number.NaN;
    let frame = 0;
    const update = () => {
      frame = 0;
      const key = inputs();
      if (key !== measured) {
        measured = key;
        rootFontPx = Number.parseFloat(view.getComputedStyle(root).fontSize);
      }
      const bounds = host.getBoundingClientRect();
      const next = {
        host: {
          x: bounds.x,
          y: bounds.y,
          width: bounds.width,
          height: bounds.height,
        },
        // The viewport hook owns keyboard and safe-area adjustments to the root.
        viewport: {
          x: 0,
          y: 0,
          width: view.innerWidth,
          height: view.innerHeight,
        },
        rootFontPx,
        shortTouchLandscape: root.hasAttribute("data-viewport-short-landscape"),
      };
      setGeometry((old) =>
        JSON.stringify(old) === JSON.stringify(next) ? old : next,
      );
    };
    // One measurement per frame, after the frame's own changes.
    const schedule = () => {
      if (!frame) frame = view.requestAnimationFrame(update);
    };
    const resize = new ResizeObserver(schedule);
    resize.observe(host);
    resize.observe(root);
    const preferences = new MutationObserver(() => {
      if (inputs() !== measured) schedule();
    });
    preferences.observe(root, {
      attributes: true,
      attributeFilter: ["style", "class", "data-viewport-short-landscape"],
    });
    view.addEventListener("resize", schedule);
    update();
    return () => {
      view.cancelAnimationFrame(frame);
      resize.disconnect();
      preferences.disconnect();
      view.removeEventListener("resize", schedule);
    };
  }, [host]);
  return {
    rowRef,
    row: host,
    shortTouchLandscape: geometry.shortTouchLandscape,
    ...deriveWorkspaceLayout({
      ...geometry,
      ...options,
      hostExcludesSafeInsets: true,
    }),
  };
}
