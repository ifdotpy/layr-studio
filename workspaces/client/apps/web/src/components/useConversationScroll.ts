import {
  useCallback,
  useLayoutEffect,
  useRef,
  useSyncExternalStore,
} from "react";
import { followsStoredAnchor } from "./conversation/transcript/historyAnchorStore";

/** Distance from the bottom, in CSS pixels, inside which the transcript keeps
 * following new content. */
export const FOLLOW_THRESHOLD_PX = 32;
/** Rounding allowance at the history start, in CSS pixels. */
export const TRANSCRIPT_HISTORY_START_PX = 1;
export function shouldFollowTranscriptTail(
  distance: number,
  followTail: boolean,
) {
  return followTail && distance < FOLLOW_THRESHOLD_PX;
}
/** Time, in milliseconds, for which wheel, touch, or key input is recent. */
const RECENT_INPUT_MS = 600;
export function useFollowState(
  subscribe: (listener: () => void) => () => void,
  getSnapshot: () => boolean,
) {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

export function useConversationScroll(
  _id: string,
  ready: boolean,
  followTail = true,
  anchorStorageKey?: string,
) {
  const scroll = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const following = useRef(true);
  const followListeners = useRef(new Set<() => void>());
  const getFollow = useCallback(() => following.current, []);
  const subscribeFollow = useCallback((listener: () => void) => {
    followListeners.current.add(listener);
    return () => {
      followListeners.current.delete(listener);
    };
  }, []);
  const updateFollow = (value: boolean) => {
    if (following.current === value) return;
    following.current = value;
    followListeners.current.forEach((listener) => listener());
  };
  const available = useRef(ready);
  available.current = ready;
  const lastTop = useRef(0);
  const lastHeight = useRef(0);
  const lastScrollInput = useRef(-Infinity);
  const hasRecentScrollInput = useCallback(
    () => performance.now() - lastScrollInput.current < RECENT_INPUT_MS,
    [],
  );
  const lastHistoryInput = useRef(-Infinity);
  const consumeHistoryScrollInput = useCallback(() => {
    if (
      !hasRecentScrollInput() ||
      lastHistoryInput.current === lastScrollInput.current
    )
      return false;
    lastHistoryInput.current = lastScrollInput.current;
    return true;
  }, [hasRecentScrollInput]);
  const remember = () => {
    const root = scroll.current;
    if (!root) return;
    lastTop.current = root.scrollTop;
    lastHeight.current = root.scrollHeight;
    root.dispatchEvent(new Event("studio-history-remember"));
  };
  const setFollow = (value: boolean) => {
    updateFollow(value);
    if (value) lastScrollInput.current = -Infinity;
    if (value)
      scroll.current?.dispatchEvent(new Event("studio-history-follow"));
    else remember();
  };
  useLayoutEffect(() => {
    lastScrollInput.current = -Infinity;
    lastHistoryInput.current = -Infinity;
    lastTop.current = 0;
    lastHeight.current = 0;
    updateFollow(followsStoredAnchor(anchorStorageKey));
  }, [_id, anchorStorageKey]);
  useLayoutEffect(() => {
    if (!followTail) updateFollow(false);
    else if (following.current)
      scroll.current?.dispatchEvent(new Event("studio-history-follow"));
  }, [followTail]);

  useLayoutEffect(() => {
    const root = scroll.current;
    const body = content.current;
    if (!root || !body) return;
    const input = (event: Event) => {
      if (event instanceof KeyboardEvent) {
        if (
          ![
            "ArrowUp",
            "ArrowDown",
            "PageUp",
            "PageDown",
            "Home",
            "End",
            " ",
          ].includes(event.key)
        )
          return;
        if (
          (event.target as HTMLElement)?.closest(
            "input, textarea, [contenteditable=true]",
          )
        )
          return;
      }
      if (event.type === "pointerdown" && event.target !== root) return;
      if (
        (event instanceof WheelEvent && event.deltaY < 0) ||
        (event instanceof KeyboardEvent &&
          (["ArrowUp", "PageUp", "Home"].includes(event.key) ||
            (event.key === " " && event.shiftKey)))
      )
        updateFollow(false);
      lastScrollInput.current = performance.now();
    };
    for (const type of ["wheel", "touchmove", "keydown", "pointerdown"])
      root.addEventListener(type, input, { passive: true });
    return () => {
      for (const type of ["wheel", "touchmove", "keydown", "pointerdown"])
        root.removeEventListener(type, input);
    };
  }, [_id]);

  const onScroll = () => {
    const root = scroll.current;
    if (!root || !available.current || root.scrollTop === lastTop.current)
      return;
    if (
      following.current &&
      !hasRecentScrollInput() &&
      root.scrollHeight !== lastHeight.current
    ) {
      remember();
      return;
    }
    const atBottom = shouldFollowTranscriptTail(
      root.scrollHeight - root.scrollTop - root.clientHeight,
      followTail,
    );
    updateFollow(
      atBottom &&
        (following.current ||
          (root.scrollTop > lastTop.current &&
            root.scrollHeight === lastHeight.current)),
    );
    remember();
  };
  const revealRequest = useCallback((node: HTMLElement, offset = 12) => {
    updateFollow(false);
    scroll.current?.dispatchEvent(
      new CustomEvent("studio-history-position", { detail: { node, offset } }),
    );
  }, []);
  const getAnchorId = () => {
    const detail: { id: string | null } = { id: null };
    scroll.current?.dispatchEvent(
      new CustomEvent("studio-history-anchor-id", { detail }),
    );
    return detail.id;
  };
  return {
    scroll,
    content,
    getFollow,
    subscribeFollow,
    setFollow,
    onScroll,
    hasRecentScrollInput,
    consumeHistoryScrollInput,
    remember,
    revealRequest,
    getAnchorId,
  };
}
