import {
  Fragment,
  memo,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { messageRenderKey } from "../../message-delivery/messageDelivery";
import { shouldFollowTranscriptTail } from "../../useConversationScroll";
import type { HistoryGroup } from "../../turnHistoryModel";
import {
  HISTORY_WINDOW_THRESHOLD,
  historyWindowIndices,
  historyOffsets,
  historyWindowAnchor,
  historyWindowRange,
  historyRowsCanRestoreAnchor,
} from "./historyWindowModel";

import { anchors, type WindowAnchor } from "./historyAnchorStore";
/** CSS pixels represented by one wheel line. */
const WHEEL_LINE_PX = 40;
/** Rounding allowance, in CSS pixels, for a recent input movement. */
const INPUT_SLACK_PX = 4;

const HistoryRow = memo(function HistoryRow({
  row,
  render,
  revealedMessage,
}: {
  row: HistoryGroup;
  render: (row: HistoryGroup, revealedMessage?: string) => ReactNode;
  revealedMessage?: string | undefined;
}) {
  return render(row, revealedMessage);
});

/** Owns the virtualized viewport and its paging anchor. */
export default function HistoryWindow({
  rows,
  scrollContainer,
  render,
  storageKey,
  rememberScroll,
  getFollow,
  followTail = true,
  hasRecentScrollInput,
  requiredRows,
}: {
  rows: HistoryGroup[];
  scrollContainer?: RefObject<HTMLDivElement | null> | undefined;
  render: (row: HistoryGroup, revealedMessage?: string) => ReactNode;
  storageKey: string;
  rememberScroll?: (() => void) | undefined;
  getFollow?: (() => boolean) | undefined;
  followTail?: boolean;
  hasRecentScrollInput?: (() => boolean) | undefined;
  /** Rows with pending user input must remain reachable while history is virtualized. */
  requiredRows?: string[];
}) {
  const body = useRef<HTMLDivElement>(null);
  const pointerHeld = useRef(false);
  const pendingRestore = useRef<WindowAnchor | "tail" | null>(null);
  const scheduleUpdate = useRef<() => void>(() => {});
  const heights = useRef(new Map<string, number>());
  const details = useRef(new Map<string, boolean>());
  const [revision, setRevision] = useState(0);
  const offsets = useMemo(
    () => historyOffsets(rows, heights.current),
    [rows, revision],
  );
  const [visible, setVisible] = useState<string[]>(() =>
    rows.slice(-12).map((row) => row.id),
  );
  const [initialAnchor] = useState(() => anchors.get(storageKey) || null);
  const storedAnchor = useRef(initialAnchor);
  const viewportAnchor = useRef<WindowAnchor | null>(initialAnchor);
  const restoringAnchor = useRef(initialAnchor?.id || null);
  const restoreViewportAnchor = useRef<(anchor: WindowAnchor) => void>(
    () => {},
  );
  const persistViewportAnchor = useRef<() => void>(() => {});
  const captureAnchorAfterUpdate = useRef(false);
  const preserveUnseen = useRef<() => void>(() => {});
  const previousRows = useRef(rows);
  const previousOffsets = useRef(offsets);
  const [requested, setRequested] = useState<string | null>(
    () => storedAnchor.current?.id || null,
  );
  const getFollowRef = useRef(getFollow);
  getFollowRef.current = getFollow;
  const followTailRef = useRef(followTail);
  followTailRef.current = followTail;
  const rememberLatest = useRef(rememberScroll);
  rememberLatest.current = rememberScroll;
  const [revealedMessage, setRevealedMessage] = useState<string>();
  const [pinned, setPinned] = useState<string[]>([]);
  const active =
    !!scrollContainer &&
    (rows.length > HISTORY_WINDOW_THRESHOLD ||
      rows.reduce((count, row) => count + row.items.length, 0) > 256);
  const indices = useMemo(() => {
    return historyWindowIndices(
      rows,
      visible,
      [...pinned, ...(requiredRows || [])],
      [requested, viewportAnchor.current?.id],
    );
  }, [rows, visible, pinned, requested, requiredRows]);
  const previousIndices = useRef(indices);
  const rowsById = useMemo(
    () => new Map(rows.map((row) => [row.id, row])),
    [rows],
  );
  const latest = useRef({ rows, offsets, rowsById });
  latest.current = { rows, offsets, rowsById };
  useEffect(() => {
    const root = scrollContainer?.current;
    const node = body.current;
    if (!root || !node) return;
    let frame = 0;
    let viewportRestoreFrame = 0;
    let programmaticTop: number | null = null;
    let inputTop: number | null = null;
    let inputReach = 0;
    const resetAnchor = () => {
      if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
      viewportRestoreFrame = 0;
      captureAnchorAfterUpdate.current = false;
      viewportAnchor.current = null;
      storedAnchor.current = null;
      restoringAnchor.current = null;
      setRequested(null);
      setRevealedMessage(undefined);
      anchors.set(storageKey, null);
    };
    const reveal = (event: Event) => {
      const id: unknown = (event as CustomEvent).detail;
      if (typeof id !== "string") return;
      const match = historyWindowAnchor(latest.current.rows, id);
      if (match) {
        setRequested(match.row.id);
        setRevealedMessage(match.item.id);
      }
    };
    root.addEventListener("studio-history-reveal", reveal);
    root.addEventListener("studio-history-reset-anchor", resetAnchor);
    const persistAnchor = (force = false) => {
      const bounds = root.getBoundingClientRect();
      const row = Array.from(
        node.querySelectorAll<HTMLElement>("[data-history-row]"),
      ).find((row) => {
        const box = row.getBoundingClientRect();
        return box.bottom > bounds.top && box.top < bounds.bottom;
      });
      if (
        !force &&
        shouldFollowTranscriptTail(
          root.scrollHeight - root.scrollTop - root.clientHeight,
          followTailRef.current,
        )
      ) {
        viewportAnchor.current = null;
        anchors.set(storageKey, null);
      } else if (row) {
        const metadata = latest.current.rowsById.get(row.dataset.historyRow!);
        if (!metadata) return;
        const visible = Array.from(
          row.querySelectorAll<HTMLElement>(
            "[data-message]:not([data-lazy-message])",
          ),
        ).find((message) => {
          const box = message.getBoundingClientRect();
          return (
            box.height > 0 && box.bottom > bounds.top && box.top < bounds.bottom
          );
        });
        const item = visible
          ? metadata.items.find((item) => item.id === visible.dataset.message)
          : metadata.items[0];
        const paragraph =
          visible &&
          Array.from(visible.querySelectorAll<HTMLElement>("p, pre, li")).find(
            (part) => {
              const box = part.getBoundingClientRect();
              return (
                box.height > 0 &&
                box.bottom > bounds.top &&
                box.top < bounds.bottom
              );
            },
          );
        const anchor: WindowAnchor = item
          ? {
              id: messageRenderKey(item),
              kind: visible ? "message" : "row",
              offset:
                (paragraph || visible || row).getBoundingClientRect().top -
                bounds.top,
              ...(paragraph
                ? {
                    paragraph: Array.from(
                      visible!.querySelectorAll<HTMLElement>("p, pre, li"),
                    ).indexOf(paragraph),
                    fallbackOffset:
                      (visible || row).getBoundingClientRect().top - bounds.top,
                  }
                : {}),
            }
          : {
              id: row.dataset.historyRow!,
              kind: "row",
              offset: row.getBoundingClientRect().top - bounds.top,
            };
        viewportAnchor.current = anchor;
        anchors.set(storageKey, anchor);
      } else if (force && latest.current.rows.length) {
        const nodeTop = node.getBoundingClientRect().top - bounds.top;
        const index = Math.min(
          latest.current.rows.length - 1,
          Math.max(
            0,
            latest.current.offsets.findLastIndex(
              (offset) => offset <= -nodeTop,
            ),
          ),
        );
        const estimated = latest.current.rows[index]!;
        const anchor: WindowAnchor = {
          id: messageRenderKey(estimated.items[0]),
          kind: "row",
          offset: nodeTop + latest.current.offsets[index]!,
        };
        viewportAnchor.current = anchor;
        anchors.set(storageKey, anchor);
        setRequested(estimated.id);
      }
    };
    const update = () => {
      frame = 0;
      const { rows: current, offsets: positions } = latest.current;
      const top =
        root.scrollTop -
        (node.getBoundingClientRect().top -
          root.getBoundingClientRect().top +
          root.scrollTop);
      const range = historyWindowRange(positions, top, root.clientHeight);
      const ids = current.slice(range.start, range.end).map((row) => row.id);
      setVisible((old) =>
        old.length === ids.length && old.every((id, index) => id === ids[index])
          ? old
          : ids,
      );
      setRequested((id) =>
        id &&
        restoringAnchor.current !== id &&
        ids.includes(historyWindowAnchor(current, id)?.row.id || id)
          ? null
          : id,
      );
      if (captureAnchorAfterUpdate.current) {
        captureAnchorAfterUpdate.current = false;
        requestAnimationFrame(() => {
          if (programmaticTop === null) persistViewportAnchor.current();
        });
      }
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    scheduleUpdate.current = schedule;
    const scrolled = () => {
      if (programmaticTop === root.scrollTop) {
        schedule();
        return;
      }
      programmaticTop = null;
      if (hasRecentScrollInput?.()) {
        if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
        viewportRestoreFrame = 0;
        restoringAnchor.current = null;
        viewportAnchor.current = null;
        storedAnchor.current = null;
        // persistAnchor below writes the actual position. A provisional null
        // doubles durable writes and prevents identical-anchor deduplication.
      }
      inputTop = null;
      captureAnchorAfterUpdate.current = true;
      schedule();
      persistAnchor();
      if (
        pointerHeld.current &&
        pendingRestore.current &&
        pendingRestore.current !== "tail"
      )
        pendingRestore.current = viewportAnchor.current;
    };
    persistViewportAnchor.current = persistAnchor;
    const capturePageAnchor = (event: Event) => {
      inputTop = null;
      persistAnchor(true);
      const anchor = viewportAnchor.current;
      const detail = (
        event as CustomEvent<{ id: string | null; endId: string | null }>
      ).detail;
      detail.id = anchor?.id || null;
      const bounds = root.getBoundingClientRect();
      const visible = Array.from(
        node.querySelectorAll<HTMLElement>("[data-message]"),
      ).filter((message) => {
        const box = message.getBoundingClientRect();
        return (
          box.height > 0 && box.bottom > bounds.top && box.top < bounds.bottom
        );
      });
      detail.endId = visible.at(-1)?.dataset.message || null;
      if (anchor) {
        restoringAnchor.current = anchor.id;
      }
    };
    root.addEventListener("studio-history-page-anchor", capturePageAnchor);
    const anchorId = (event: Event) => {
      persistAnchor(true);
      (event as CustomEvent<{ id: string | null }>).detail.id =
        viewportAnchor.current?.id || null;
    };
    const follow = () => {
      if (pointerHeld.current) {
        pendingRestore.current = "tail";
        return;
      }
      root.scrollTop = root.scrollHeight;
      rememberLatest.current?.();
      persistAnchor();
    };
    const position = (event: Event) => {
      const { node: target, offset } = (
        event as CustomEvent<{ node: HTMLElement; offset: number }>
      ).detail;
      if (!root.contains(target) || pointerHeld.current) return;
      root.scrollTop +=
        target.getBoundingClientRect().top -
        root.getBoundingClientRect().top -
        offset;
      persistAnchor(true);
    };
    root.addEventListener("studio-history-anchor-id", anchorId);
    root.addEventListener("studio-history-follow", follow);
    root.addEventListener("studio-history-position", position);
    const remember = () => {
      if (programmaticTop === null) persistAnchor();
    };
    root.addEventListener("studio-history-remember", remember);
    let focusOwner: string | undefined;
    const pin = () => {
      const selection = document.getSelection();
      const selected =
        selection &&
        !selection.isCollapsed &&
        node.contains(selection.anchorNode);
      const focused =
        node.contains(document.activeElement) &&
        document.activeElement !== document.body;
      const focusedRow = focused
        ? document.activeElement?.closest<HTMLElement>("[data-history-row]")
            ?.dataset.historyRow
        : undefined;
      if (focusedRow) focusOwner = focusedRow;
      const modal =
        !focused &&
        !!document.activeElement?.closest('[role="dialog"]') &&
        focusOwner;
      if (!focused && !modal) focusOwner = undefined;
      setPinned((old) => {
        if (!selected && !focused && !modal) return old.length ? [] : old;
        const ids = Array.from(
          node.querySelectorAll<HTMLElement>("[data-history-row]"),
        )
          .filter((row) =>
            selected
              ? selection.containsNode(row, true)
              : row.contains(document.activeElement),
          )
          .map((row) => row.dataset.historyRow!);
        if (modal && !ids.includes(modal)) ids.push(modal);
        return old.length === ids.length &&
          old.every((id, index) => id === ids[index])
          ? old
          : ids;
      });
    };
    const cancelRestoreOnInput = (event: Event) => {
      if (event.type === "pointerdown") pointerHeld.current = true;
      if (
        event instanceof KeyboardEvent &&
        (![
          "ArrowUp",
          "ArrowDown",
          "PageUp",
          "PageDown",
          "Home",
          "End",
          " ",
        ].includes(event.key) ||
          (event.target as HTMLElement)?.closest(
            "input, textarea, [contenteditable=true]",
          ))
      )
        return;
      programmaticTop = null;
      persistAnchor(true);
      inputTop = root.scrollTop;
      inputReach =
        event instanceof WheelEvent
          ? Math.abs(event.deltaY) *
            (event.deltaMode === 1
              ? WHEEL_LINE_PX
              : event.deltaMode === 2
                ? root.clientHeight
                : 1)
          : Infinity;
      if (!viewportRestoreFrame && !restoringAnchor.current) return;
      if (pointerHeld.current) pendingRestore.current = viewportAnchor.current;
      if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
      viewportRestoreFrame = 0;
      restoringAnchor.current = null;
      persistAnchor();
      update();
    };
    const releasePointer = () => {
      if (!pointerHeld.current) return;
      pointerHeld.current = false;
      const pending = pendingRestore.current;
      pendingRestore.current = null;
      const releasedTop = root.scrollTop;
      viewportRestoreFrame = requestAnimationFrame(() => {
        viewportRestoreFrame = 0;
        if (root.scrollTop !== releasedTop) return;
        if (pending === "tail") follow();
        else if (pending) restoreViewportAnchor.current(pending);
      });
    };
    window.addEventListener("pointerup", releasePointer);
    window.addEventListener("pointercancel", releasePointer);
    root.addEventListener("scroll", scrolled, { passive: true });
    for (const type of [
      "wheel",
      "touchstart",
      "touchmove",
      "keydown",
      "pointerdown",
    ])
      root.addEventListener(type, cancelRestoreOnInput, { passive: true });
    document.addEventListener("selectionchange", pin);
    const focusChanged = () => queueMicrotask(pin);
    document.addEventListener("focusin", focusChanged);
    document.addEventListener("focusout", focusChanged);
    const preserve = () => {
      schedule();
      if (
        followTailRef.current &&
        getFollowRef.current?.() &&
        !restoringAnchor.current &&
        !pendingRestore.current
      )
        follow();
      else if (hasRecentScrollInput?.()) {
        preserveUnseen.current();
      } else if (viewportAnchor.current)
        restoreViewportAnchor.current(viewportAnchor.current);
    };
    const content = node.closest(".message-content");
    const observer = new ResizeObserver(preserve);
    observer.observe(root);
    observer.observe(node);
    if (content) observer.observe(content);
    const mutations = new MutationObserver(preserve);
    mutations.observe(node, { childList: true, subtree: true });
    const restoreAnchor = (anchor: WindowAnchor, afterRestore?: () => void) => {
      if (pointerHeld.current) {
        pendingRestore.current = anchor;
        return;
      }
      if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
      let attempts = 0;
      const retryTop = root.scrollTop;
      const measure = () => {
        if (pointerHeld.current) {
          pendingRestore.current = anchor;
          return;
        }
        if (viewportRestoreFrame && root.scrollTop !== retryTop) {
          viewportRestoreFrame = 0;
          afterRestore?.();
          return;
        }
        viewportRestoreFrame = 0;
        const resolved = historyWindowAnchor(latest.current.rows, anchor.id);
        const row = resolved
          ? Array.from(
              node.querySelectorAll<HTMLElement>("[data-history-row]"),
            ).find(
              (candidate) => candidate.dataset.historyRow === resolved.row.id,
            )
          : undefined;
        const message =
          row && resolved
            ? Array.from(
                row.querySelectorAll<HTMLElement>("[data-message]"),
              ).find(
                (candidate) =>
                  candidate.dataset.message === resolved.item.id ||
                  (!!resolved.item.sourceId &&
                    candidate.dataset.sourceMessage === resolved.item.sourceId),
              )
            : undefined;
        if (anchor.kind === "message" && message) {
          for (
            let parent = message.parentElement;
            parent && parent !== node;
            parent = parent.parentElement
          )
            if (parent instanceof HTMLDetailsElement) parent.open = true;
        }
        const visibleMessage =
          message &&
          !message.hasAttribute("data-lazy-message") &&
          message.getBoundingClientRect().height > 0;
        if (
          anchor.kind === "message" &&
          resolved &&
          !visibleMessage &&
          attempts++ < 20
        ) {
          viewportRestoreFrame = requestAnimationFrame(measure);
          return;
        }
        const paragraph =
          anchor.paragraph === undefined
            ? undefined
            : message?.querySelectorAll<HTMLElement>("p, pre, li")[
                anchor.paragraph
              ];
        const target = paragraph?.getClientRects().length
          ? paragraph
          : anchor.kind === "message" && visibleMessage
            ? message
            : row;
        if (!target) {
          afterRestore?.();
          schedule();
          return;
        }
        if (anchor.kind === "message" && target === row && attempts++ < 20) {
          viewportRestoreFrame = requestAnimationFrame(measure);
          return;
        }
        const top =
          target.getBoundingClientRect().top - root.getBoundingClientRect().top;
        root.scrollTop +=
          top -
          (target === paragraph && !!paragraph
            ? anchor.offset
            : (anchor.fallbackOffset ?? anchor.offset));
        programmaticTop = root.scrollTop;
        rememberLatest.current?.();
        afterRestore?.();
        schedule();
      };
      measure();
    };
    preserveUnseen.current = () => {
      const anchor = viewportAnchor.current;
      if (inputTop === null || !anchor) return;
      const moved = root.scrollTop - inputTop;
      if (Math.abs(moved) > inputReach + INPUT_SLACK_PX) return;
      anchor.offset -= moved;
      if (anchor.fallbackOffset !== undefined) anchor.fallbackOffset -= moved;
      restoreAnchor(anchor);
      inputTop = root.scrollTop;
    };
    restoreViewportAnchor.current = (anchor) =>
      restoreAnchor(anchor, () => {
        if (restoringAnchor.current === anchor.id)
          restoringAnchor.current = null;
      });
    const anchor = storedAnchor.current;
    storedAnchor.current = null;
    if (
      anchor &&
      typeof anchor.id === "string" &&
      Number.isFinite(anchor.offset)
    ) {
      const match = historyWindowAnchor(latest.current.rows, anchor.id);
      if (anchor.kind === "message" && match) setRevealedMessage(match.item.id);
      restoringAnchor.current = anchor.id;
      restoreViewportAnchor.current(anchor);
    } else {
      if (followTailRef.current && getFollowRef.current?.()) follow();
      update();
    }
    return () => {
      window.removeEventListener("pointerup", releasePointer);
      window.removeEventListener("pointercancel", releasePointer);
      pointerHeld.current = false;
      pendingRestore.current = null;
      preserveUnseen.current = () => {};
      scheduleUpdate.current = () => {};
      restoreViewportAnchor.current = () => {};
      persistViewportAnchor.current = () => {};
      if (frame) cancelAnimationFrame(frame);
      if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
      observer.disconnect();
      mutations.disconnect();
      root.removeEventListener("studio-history-anchor-id", anchorId);
      root.removeEventListener("studio-history-follow", follow);
      root.removeEventListener("studio-history-position", position);
      root.removeEventListener("studio-history-remember", remember);
      root.removeEventListener("scroll", scrolled);
      root.removeEventListener("studio-history-page-anchor", capturePageAnchor);
      for (const type of [
        "wheel",
        "touchstart",
        "touchmove",
        "keydown",
        "pointerdown",
      ])
        root.removeEventListener(type, cancelRestoreOnInput);
      root.removeEventListener("studio-history-reveal", reveal);
      root.removeEventListener("studio-history-reset-anchor", resetAnchor);
      document.removeEventListener("selectionchange", pin);
      document.removeEventListener("focusin", focusChanged);
      document.removeEventListener("focusout", focusChanged);
    };
  }, [hasRecentScrollInput, scrollContainer, storageKey]);
  useLayoutEffect(() => {
    preserveUnseen.current();
    scheduleUpdate.current();
    const offsetsChanged = previousOffsets.current !== offsets;
    const indicesChanged = previousIndices.current !== indices;
    previousIndices.current = indices;
    previousOffsets.current = offsets;
    if (previousRows.current !== rows) {
      const previous = previousRows.current;
      previousRows.current = rows;
      if (
        viewportAnchor.current &&
        (restoringAnchor.current === viewportAnchor.current.id ||
          historyRowsCanRestoreAnchor(previous, rows))
      )
        restoreViewportAnchor.current(viewportAnchor.current);
    } else if (
      (offsetsChanged || indicesChanged) &&
      viewportAnchor.current &&
      !getFollowRef.current?.()
    )
      restoreViewportAnchor.current(viewportAnchor.current);
  }, [rows, offsets, indices]);
  useLayoutEffect(() => {
    const node = body.current;
    if (!node) return;
    const restored = new WeakSet<HTMLDetailsElement>();
    const key = (detail: HTMLDetailsElement) => {
      const row = detail.closest<HTMLElement>("[data-history-row]");
      const message = detail.closest<HTMLElement>("[data-message]");
      if (detail.classList.contains("turn-work")) return null;
      if (!row && !message) return null;
      const parent = message || row!;
      return `${parent.dataset.message || row?.dataset.historyRow}:${Array.from(parent.querySelectorAll("details")).indexOf(detail)}:${detail.className}`;
    };
    const restore = () => {
      for (const detail of node.querySelectorAll<HTMLDetailsElement>(
        "details",
      )) {
        if (restored.has(detail)) continue;
        restored.add(detail);
        const id = key(detail);
        const open = id && details.current.get(id);
        if (typeof open === "boolean" && detail.open !== open)
          detail.open = open;
      }
    };
    const toggled = (event: Event) => {
      if (!(event.target instanceof HTMLDetailsElement)) return;
      const id = key(event.target);
      if (id) details.current.set(id, event.target.open);
    };
    node.addEventListener("toggle", toggled, true);
    restore();
    const changes = new MutationObserver(restore);
    changes.observe(node, { childList: true, subtree: true });
    const observer = new ResizeObserver((entries) => {
      let changed = false;
      for (const entry of entries) {
        const row = entry.target as HTMLElement;
        const id = row.dataset.historyRow!;
        const height = row.getBoundingClientRect().height;
        if (heights.current.get(id) !== height) {
          heights.current.set(id, height);
          changed = true;
        }
      }
      if (changed) setRevision((value) => value + 1);
    });
    if (active)
      node
        .querySelectorAll<HTMLElement>("[data-history-row]")
        .forEach((row) => observer.observe(row));
    return () => {
      observer.disconnect();
      changes.disconnect();
      node.removeEventListener("toggle", toggled, true);
    };
  }, [active, indices]);
  if (!active)
    return (
      <div ref={body} className="history-window-static">
        {rows.map((row) => (
          <div
            key={row.id}
            data-history-row={row.id}
            className="history-window-row"
          >
            {render(row, revealedMessage)}
          </div>
        ))}
      </div>
    );
  let previous = 0;
  return (
    <div ref={body} className="history-window" data-history-window>
      {indices.map((index) => {
        // Indices come from rows; offsets contains each row boundary plus its end.
        const row = rows[index]!;
        const gap = offsets[index]! - offsets[previous]!;
        previous = index + 1;
        return (
          <Fragment key={row.id}>
            {gap > 0 && (
              <div
                className="history-window-spacer"
                style={{ height: gap }}
                aria-hidden="true"
              />
            )}
            <div className="history-window-row" data-history-row={row.id}>
              <HistoryRow
                row={row}
                render={render}
                revealedMessage={revealedMessage}
              />
            </div>
          </Fragment>
        );
      })}
      <div
        className="history-window-spacer"
        style={{ height: offsets[rows.length]! - offsets[previous]! }}
        aria-hidden="true"
      />
    </div>
  );
}
