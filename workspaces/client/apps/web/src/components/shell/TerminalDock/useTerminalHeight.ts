import { useLayoutEffect, useState, type RefObject } from "react";
import { terminalHeight, type TerminalHeightBounds } from "./terminalHeight.ts";

const pixels = (value: string) => Number.parseFloat(value) || 0;

type Geometry = { available: number; viewport: number; rootFont: number };

/** Read unreserved host space; the dock reservation must not feed its own cap. */
export function useTerminalHeight(
  dock: RefObject<HTMLElement | null>,
  preferred: unknown,
  opened: boolean,
  bounds: (rootFont: number) => TerminalHeightBounds,
) {
  const [geometry, setGeometry] = useState<Geometry>(() => ({
    available: window.innerHeight,
    viewport: window.innerHeight,
    rootFont: Number.parseFloat(
      getComputedStyle(document.documentElement).fontSize,
    ),
  }));
  useLayoutEffect(() => {
    const parent = dock.current?.parentElement;
    if (!parent) return;
    let frame = 0;
    const observer = new ResizeObserver(() => schedule());
    let observed = new Set<Element>();
    const measure = () => {
      frame = 0;
      const workspace = parent.querySelector<HTMLElement>(".workspace");
      const header = workspace?.querySelector<HTMLElement>(".workspace-header");
      const conversation =
        workspace?.querySelector<HTMLElement>("#conversation");
      // The viewport surface positions the dock inside safe areas and keyboard bounds.
      // Its bottom is independent of the terminal height, so no viewport delta is repeated.
      const bottom =
        dock.current?.getBoundingClientRect().bottom ?? window.innerHeight;
      let reserved = header?.getBoundingClientRect().height ?? 0;
      const targets = new Set<Element>([document.documentElement]);
      if (workspace) targets.add(workspace);
      if (header) targets.add(header);
      if (conversation) {
        for (const element of conversation.children) {
          if (
            !(element instanceof HTMLElement) ||
            element.classList.contains("jump-slot")
          )
            continue;
          const style = getComputedStyle(element);
          if (
            style.display === "none" ||
            style.position === "absolute" ||
            style.position === "fixed"
          )
            continue;
          const verticalBorder =
            pixels(style.borderTopWidth) + pixels(style.borderBottomWidth);
          let height = element.getBoundingClientRect().height;
          if (element.id === "messages")
            height =
              pixels(style.paddingTop) +
              pixels(style.paddingBottom) +
              verticalBorder;
          if (element.id === "composer")
            height = Math.max(height, element.scrollHeight + verticalBorder);
          reserved +=
            height + pixels(style.marginTop) + pixels(style.marginBottom);
          targets.add(element);
        }
      }
      for (const element of observed) {
        if (!targets.has(element)) observer.unobserve(element);
      }
      for (const element of targets) {
        if (!observed.has(element)) observer.observe(element);
      }
      observed = targets;
      // Replaced containers are new children of these; their content is not.
      structure.disconnect();
      structure.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["style", "class"],
      });
      for (const node of [parent, workspace, conversation])
        if (node) structure.observe(node, { childList: true });
      const hostTop = Math.max(0, workspace?.getBoundingClientRect().top ?? 0);
      const next = {
        available: Math.max(0, bottom - hostTop - reserved),
        viewport: Math.max(0, bottom - hostTop),
        rootFont: Number.parseFloat(
          getComputedStyle(document.documentElement).fontSize,
        ),
      };
      setGeometry((old) =>
        Object.values(old).every((value, i) => value === Object.values(next)[i])
          ? old
          : next,
      );
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(measure);
    };
    // A chat switch can replace the measured controls without a window resize.
    // Only the measured containers' children matter: watching the whole app
    // subtree ran this measurement for every streamed token and keystroke.
    const structure = new MutationObserver(schedule);
    window.addEventListener("resize", schedule);
    window.visualViewport?.addEventListener("resize", schedule);
    window.visualViewport?.addEventListener("scroll", schedule);
    document.fonts?.addEventListener("loadingdone", schedule);
    measure();
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      structure.disconnect();
      window.removeEventListener("resize", schedule);
      window.visualViewport?.removeEventListener("resize", schedule);
      window.visualViewport?.removeEventListener("scroll", schedule);
      document.fonts?.removeEventListener("loadingdone", schedule);
    };
  }, [dock]);
  return terminalHeight({
    ...geometry,
    preferred,
    opened,
    bounds: bounds(geometry.rootFont),
  });
}
