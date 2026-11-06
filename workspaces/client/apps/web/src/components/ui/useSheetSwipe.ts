import { useRef, type PointerEvent } from "react";

/** Drag distance that closes a sheet, in CSS px. */
export const SHEET_CLOSE_DISTANCE = 80;
/** Downward speed that closes a sheet after a short flick, in px per ms. */
export const SHEET_CLOSE_SPEED = 0.5;

/** Whether a downward drag of `distance` px over `ms` ms closes the sheet. */
export const swipeCloses = (distance: number, ms: number) =>
  distance > SHEET_CLOSE_DISTANCE ||
  (distance > 0 && distance / Math.max(ms, 1) > SHEET_CLOSE_SPEED);

/**
 * Handlers for a bottom sheet's grip: the sheet follows a downward drag and
 * closes past the distance or on a flick; a shorter drag springs back.
 */
export function useSheetSwipe(onClose: () => void) {
  const drag = useRef<{ y: number; at: number; sheet: HTMLElement } | null>(
    null,
  );
  const end = (event: PointerEvent<HTMLElement>, cancelled: boolean) => {
    const current = drag.current;
    drag.current = null;
    if (!current) return;
    const distance = Math.max(0, event.clientY - current.y);
    current.sheet.style.transition = "";
    current.sheet.style.transform = "";
    if (!cancelled && swipeCloses(distance, performance.now() - current.at))
      onClose();
  };
  return {
    onPointerDown(event: PointerEvent<HTMLElement>) {
      const sheet = event.currentTarget.closest<HTMLElement>(
        ".mantine-Drawer-content",
      );
      if (!sheet || event.button !== 0) return;
      drag.current = { y: event.clientY, at: performance.now(), sheet };
      sheet.style.transition = "none";
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    onPointerMove(event: PointerEvent<HTMLElement>) {
      const current = drag.current;
      if (!current) return;
      const distance = Math.max(0, event.clientY - current.y);
      current.sheet.style.transform = `translateY(${distance}px)`;
    },
    onPointerUp: (event: PointerEvent<HTMLElement>) => end(event, false),
    onPointerCancel: (event: PointerEvent<HTMLElement>) => end(event, true),
  };
}
