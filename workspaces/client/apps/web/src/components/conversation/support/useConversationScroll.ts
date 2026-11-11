import { makeAutoObservable, reaction } from "mobx";
import { useMemo, useRef } from "react";
import { followsStoredAnchor } from "./historyAnchorStore.ts";

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

export function useConversationScroll(
  _id: string,
  ready: boolean,
  followTail = true,
  anchorStorageKey?: string,
) {
  const values = useRef({ ready, followTail });
  values.current = { ready, followTail };
  return useMemo(
    () =>
      createConversationScroll(
        () => values.current.ready,
        () => values.current.followTail,
        anchorStorageKey,
      ),
    [_id, anchorStorageKey],
  );
}

export function createConversationScroll(
  ready: () => boolean,
  followTail: () => boolean,
  anchorStorageKey?: string,
) {
  const scroll = { current: null as HTMLDivElement | null };
  const content = { current: null as HTMLDivElement | null };
  const following = makeAutoObservable(
    {
      current: followsStoredAnchor(anchorStorageKey),
      update(value: boolean) {
        this.current = value;
      },
    },
    {},
    { autoBind: true },
  );
  const getFollow = () => following.current && followTail();
  const updateFollow = following.update.bind(following);
  const lastTop = { current: 0 };
  const lastHeight = { current: 0 };
  const lastScrollInput = { current: -Infinity };
  const hasRecentScrollInput = () =>
    performance.now() - lastScrollInput.current < RECENT_INPUT_MS;
  const lastHistoryInput = { current: -Infinity };
  const consumeHistoryScrollInput = () => {
    if (
      !hasRecentScrollInput() ||
      lastHistoryInput.current === lastScrollInput.current
    )
      return false;
    lastHistoryInput.current = lastScrollInput.current;
    return true;
  };
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
  const bindInput = (root: HTMLDivElement | null) => {
    if (!root) return () => {};
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
  };
  let unbind = () => {};
  let root: HTMLDivElement | null = null;
  Object.defineProperty(scroll, "current", {
    get: () => root,
    set: (next: HTMLDivElement | null) => {
      unbind();
      root = next;
      unbind = bindInput(next);
    },
  });

  const onScroll = () => {
    const root = scroll.current;
    if (!root || !ready() || root.scrollTop === lastTop.current) return;
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
      followTail(),
    );
    updateFollow(
      atBottom &&
        (following.current ||
          (root.scrollTop > lastTop.current &&
            root.scrollHeight === lastHeight.current)),
    );
    remember();
  };
  const revealRequest = (node: HTMLElement, offset = 12) => {
    updateFollow(false);
    scroll.current?.dispatchEvent(
      new CustomEvent("studio-history-position", { detail: { node, offset } }),
    );
  };
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
    subscribeFollow: (listener: () => void): (() => void) =>
      reaction(getFollow, listener),
    setFollow,
    onScroll,
    hasRecentScrollInput,
    consumeHistoryScrollInput,
    remember,
    revealRequest,
    getAnchorId,
  };
}
