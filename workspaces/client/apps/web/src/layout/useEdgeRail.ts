import {
  hasTouchCapability,
  touchPointerMedia,
} from "../hooks/touchCapability.ts";
import { layoutValues } from "./layout-contract.generated.ts";
import {
  edgeRailGeometry,
  type EdgeRailGeometry,
  type EdgeRailMetrics,
  type EdgeRailViewport,
} from "./edgeRailPolicy.ts";

export const layoutEdgeRailMetrics: EdgeRailMetrics = {
  minimumInset: layoutValues.edgeRail.minInset,
  statusReserve: layoutValues.edgeRail.statusReserve,
  targetSize: layoutValues.edgeRail.buttonSize,
  statusColumnCenter: layoutValues.edgeRail.statusColumnCenter,
};

type RootStyle = Pick<
  CSSStyleDeclaration,
  "height" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft"
>;

/** Read resolved CSS lengths. The viewport owner supplies all compensation. */
export function readEdgeRailViewport(
  style: RootStyle,
  coarsePointer: boolean,
): EdgeRailViewport {
  return {
    coarsePointer,
    viewportHeight: Number.parseFloat(style.height),
    insets: {
      top: Number.parseFloat(style.paddingTop),
      right: Number.parseFloat(style.paddingRight),
      bottom: Number.parseFloat(style.paddingBottom),
      left: Number.parseFloat(style.paddingLeft),
    },
  };
}

export function observeEdgeRail(
  record: (geometry: EdgeRailGeometry | null) => void,
  metrics: EdgeRailMetrics = layoutEdgeRailMetrics,
) {
  const { minimumInset, statusReserve, targetSize, statusColumnCenter } =
    metrics;
  const root = document.getElementById("root");
  if (!root) return;
  const coarse = touchPointerMedia();
  let frame = 0;
  const update = () => {
    frame = 0;
    const next = edgeRailGeometry(
      readEdgeRailViewport(
        getComputedStyle(root),
        hasTouchCapability(coarse.matches, navigator.maxTouchPoints),
      ),
      { minimumInset, statusReserve, targetSize, statusColumnCenter },
    );
    record(next);
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(update);
  };
  const resize = new ResizeObserver(schedule);
  resize.observe(root);
  const mutation = new MutationObserver(schedule);
  mutation.observe(document.documentElement, { attributes: true });
  mutation.observe(root, { attributes: true });
  coarse.addEventListener("change", schedule);
  window.addEventListener("resize", schedule);
  window.addEventListener("orientationchange", schedule);
  window.visualViewport?.addEventListener("resize", schedule);
  window.visualViewport?.addEventListener("scroll", schedule);
  update();
  return () => {
    cancelAnimationFrame(frame);
    resize.disconnect();
    mutation.disconnect();
    coarse.removeEventListener("change", schedule);
    window.removeEventListener("resize", schedule);
    window.removeEventListener("orientationchange", schedule);
    window.visualViewport?.removeEventListener("resize", schedule);
    window.visualViewport?.removeEventListener("scroll", schedule);
  };
}
