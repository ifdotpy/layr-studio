import { messageRenderKey } from "../../support/messageDelivery.ts";
import { shouldFollowTranscriptTail } from "../../support/useConversationScroll.ts";
import {
  historyWindowAnchor,
  historyWindowRange,
  historyRowsCanRestoreAnchor,
} from "../../support/historyWindowModel.ts";
import {
  anchors,
  type WindowAnchor,
} from "../../support/historyAnchorStore.ts";
import type { TranscriptStore } from "../../support/TurnHistoryStore.ts";
/** CSS pixels represented by one wheel line. */
const WHEEL_LINE_PX = 40;
/** Rounding allowance, in CSS pixels, for a recent input movement. */
const INPUT_SLACK_PX = 4;
/** DOM measurements, listeners and scroll restoration for one mounted viewport. */
export function mountHistoryViewportDom(
  model: TranscriptStore,
  node: HTMLDivElement,
  root: HTMLDivElement,
) {
  const { storageKey, hasRecentScrollInput } = model.input;
  let pointerHeld = false;
  let pendingRestore: WindowAnchor | "tail" | null = null;
  let viewportAnchor = model.initialAnchor;
  model.anchor = () => viewportAnchor?.id;
  let restoringAnchor = viewportAnchor?.id || null;
  let captureAnchorAfterUpdate = false;
  let previousRows = model.rows;
  let previousOffsets = model.offsets;
  let previousIndices = model.indices;
  let stopRows = () => {};

  const events = new AbortController();
  const listen = (
    target: EventTarget,
    name: string,
    listener: EventListener,
    options?: AddEventListenerOptions,
  ) =>
    target.addEventListener(name, listener, {
      ...options,
      signal: events.signal,
    });
  let frame = 0;
  let viewportRestoreFrame = 0;
  let programmaticTop: number | null = null;
  let inputTop: number | null = null;
  let inputReach = 0;
  const resetAnchor = () => {
    if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
    viewportRestoreFrame = 0;
    captureAnchorAfterUpdate = false;
    viewportAnchor = null;
    restoringAnchor = null;
    model.request(null);
    model.reveal(undefined);
    anchors.set(storageKey, null);
  };
  const reveal = (event: Event) => {
    const id: unknown = (event as CustomEvent).detail;
    if (typeof id !== "string") return;
    const match = historyWindowAnchor(model.rows, id);
    if (match) {
      model.request(match.row.id);
      model.reveal(match.item.id);
    }
  };
  listen(root, "studio-history-reveal", reveal);
  listen(root, "studio-history-reset-anchor", resetAnchor);
  const visibleIn = (element: HTMLElement, bounds: DOMRect) => {
    const box = element.getBoundingClientRect();
    return box.height > 0 && box.bottom > bounds.top && box.top < bounds.bottom;
  };
  const persistAnchor = (force = false) => {
    const bounds = root.getBoundingClientRect();
    const row = elements(node, "[data-history-row]").find((row) =>
      visibleIn(row, bounds),
    );
    if (
      !force &&
      shouldFollowTranscriptTail(
        root.scrollHeight - root.scrollTop - root.clientHeight,
        model.input.followTail ?? true,
      )
    ) {
      viewportAnchor = null;
      anchors.set(storageKey, null);
    } else if (row) {
      const metadata = model.rowsById.get(row.dataset.historyRow!);
      if (!metadata) return;
      const visible = elements(
        row,
        "[data-message]:not([data-lazy-message])",
      ).find((message) => visibleIn(message, bounds));
      const item = visible
        ? metadata.items.find((item) => item.id === visible.dataset.message)
        : metadata.items[0];
      const paragraph =
        visible &&
        elements(visible, "p, pre, li").find((part) => visibleIn(part, bounds));
      const top = (element: HTMLElement) =>
        element.getBoundingClientRect().top - bounds.top;
      const anchor: WindowAnchor = {
        id: item ? messageRenderKey(item) : row.dataset.historyRow!,
        kind: item && visible ? "message" : "row",
        offset: top(item ? paragraph || visible || row : row),
      };
      if (item && paragraph && visible) {
        anchor.paragraph = elements(visible, "p, pre, li").indexOf(paragraph);
        anchor.fallbackOffset = top(visible);
      }
      viewportAnchor = anchor;
      anchors.set(storageKey, anchor);
    } else if (force && model.rows.length) {
      const nodeTop = node.getBoundingClientRect().top - bounds.top;
      const index = Math.min(
        model.rows.length - 1,
        Math.max(
          0,
          model.offsets.findLastIndex((offset) => offset <= -nodeTop),
        ),
      );
      const estimated = model.rows[index]!;
      const anchor: WindowAnchor = {
        id: messageRenderKey(estimated.items[0]),
        kind: "row",
        offset: nodeTop + model.offsets[index]!,
      };
      viewportAnchor = anchor;
      anchors.set(storageKey, anchor);
      model.request(estimated.id);
    }
  };
  const update = () => {
    frame = 0;
    const { rows: current, offsets: positions } = model;
    const top =
      root.getBoundingClientRect().top - node.getBoundingClientRect().top;
    const range = historyWindowRange(positions, top, root.clientHeight);
    const ids = current.slice(range.start, range.end).map((row) => row.id);
    model.setVisible(ids);
    const id = model.requested;
    model.request(
      id &&
        restoringAnchor !== id &&
        ids.includes(historyWindowAnchor(current, id)?.row.id || id)
        ? null
        : id,
    );
    if (captureAnchorAfterUpdate) {
      captureAnchorAfterUpdate = false;
      requestAnimationFrame(() => {
        if (programmaticTop === null) persistAnchor();
      });
    }
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(update);
  };
  const scrolled = () => {
    if (programmaticTop === root.scrollTop) {
      schedule();
      return;
    }
    programmaticTop = null;
    if (hasRecentScrollInput?.()) {
      if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
      viewportRestoreFrame = 0;
      restoringAnchor = null;
      viewportAnchor = null;
      // persistAnchor below writes the actual position. A provisional null
      // doubles durable writes and prevents identical-anchor deduplication.
    }
    inputTop = null;
    captureAnchorAfterUpdate = true;
    schedule();
    persistAnchor();
    if (pointerHeld && pendingRestore && pendingRestore !== "tail")
      pendingRestore = viewportAnchor;
  };
  const capturePageAnchor = (event: Event) => {
    inputTop = null;
    persistAnchor(true);
    const anchor = viewportAnchor;
    const detail = (
      event as CustomEvent<{ id: string | null; endId: string | null }>
    ).detail;
    detail.id = anchor?.id || null;
    const bounds = root.getBoundingClientRect();
    const visible = elements(node, "[data-message]").filter((message) =>
      visibleIn(message, bounds),
    );
    detail.endId = visible.at(-1)?.dataset.message || null;
    if (anchor) {
      restoringAnchor = anchor.id;
    }
  };
  listen(root, "studio-history-page-anchor", capturePageAnchor);
  const anchorId = (event: Event) => {
    persistAnchor(true);
    (event as CustomEvent<{ id: string | null }>).detail.id =
      viewportAnchor?.id || null;
  };
  const follow = () => {
    if (pointerHeld) {
      pendingRestore = "tail";
      return;
    }
    root.scrollTop = root.scrollHeight;
    model.input.rememberScroll?.();
    persistAnchor();
  };
  const position = (event: Event) => {
    const { node: target, offset } = (
      event as CustomEvent<{ node: HTMLElement; offset: number }>
    ).detail;
    if (!root.contains(target) || pointerHeld) return;
    root.scrollTop +=
      target.getBoundingClientRect().top -
      root.getBoundingClientRect().top -
      offset;
    persistAnchor(true);
  };
  listen(root, "studio-history-anchor-id", anchorId);
  listen(root, "studio-history-follow", follow);
  listen(root, "studio-history-position", position);
  const remember = () => {
    if (programmaticTop === null) persistAnchor();
  };
  listen(root, "studio-history-remember", remember);
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
    if (!selected && !focused && !modal) {
      model.setPinned([]);
      return;
    }
    const ids = elements(node, "[data-history-row]").flatMap((row) =>
      (
        selected
          ? selection.containsNode(row, true)
          : row.contains(document.activeElement)
      )
        ? [row.dataset.historyRow!]
        : [],
    );
    if (modal && !ids.includes(modal)) ids.push(modal);
    model.setPinned(ids);
  };
  const cancelRestoreOnInput = (event: Event) => {
    if (event.type === "pointerdown") pointerHeld = true;
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
    if (!viewportRestoreFrame && !restoringAnchor) return;
    if (pointerHeld) pendingRestore = viewportAnchor;
    if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
    viewportRestoreFrame = 0;
    restoringAnchor = null;
    persistAnchor();
    update();
  };
  const releasePointer = () => {
    if (!pointerHeld) return;
    pointerHeld = false;
    const pending = pendingRestore;
    pendingRestore = null;
    const releasedTop = root.scrollTop;
    viewportRestoreFrame = requestAnimationFrame(() => {
      viewportRestoreFrame = 0;
      if (root.scrollTop !== releasedTop) return;
      if (pending === "tail") follow();
      else if (pending) restoreViewportAnchor(pending);
    });
  };
  listen(window, "pointerup", releasePointer);
  listen(window, "pointercancel", releasePointer);
  listen(root, "scroll", scrolled, { passive: true });
  for (const type of [
    "wheel",
    "touchstart",
    "touchmove",
    "keydown",
    "pointerdown",
  ])
    listen(root, type, cancelRestoreOnInput, { passive: true });
  listen(document, "selectionchange", pin);
  const focusChanged = () => queueMicrotask(pin);
  listen(document, "focusin", focusChanged);
  listen(document, "focusout", focusChanged);
  const preserve = () => {
    schedule();
    if (
      (model.input.followTail ?? true) &&
      model.input.getFollow?.() &&
      !restoringAnchor &&
      !pendingRestore
    )
      follow();
    else if (hasRecentScrollInput?.()) {
      preserveUnseen();
    } else if (viewportAnchor) restoreViewportAnchor(viewportAnchor);
  };
  const content = node.closest(".message-content");
  const observer = new ResizeObserver(preserve);
  observer.observe(root);
  observer.observe(node);
  if (content) observer.observe(content);
  const mutations = new MutationObserver(preserve);
  mutations.observe(node, { childList: true, subtree: true });
  const restoreAnchor = (anchor: WindowAnchor, afterRestore?: () => void) => {
    if (pointerHeld) {
      pendingRestore = anchor;
      return;
    }
    if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
    let attempts = 0;
    const retryTop = root.scrollTop;
    const measure = () => {
      if (pointerHeld) {
        pendingRestore = anchor;
        return;
      }
      if (viewportRestoreFrame && root.scrollTop !== retryTop) {
        viewportRestoreFrame = 0;
        afterRestore?.();
        return;
      }
      viewportRestoreFrame = 0;
      const resolved = historyWindowAnchor(model.rows, anchor.id);
      const row = resolved
        ? elements(node, "[data-history-row]").find(
            (candidate) => candidate.dataset.historyRow === resolved.row.id,
          )
        : undefined;
      const message =
        row && resolved
          ? elements(row, "[data-message]").find(
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
      const top =
        target.getBoundingClientRect().top - root.getBoundingClientRect().top;
      root.scrollTop +=
        top -
        (target === paragraph && !!paragraph
          ? anchor.offset
          : (anchor.fallbackOffset ?? anchor.offset));
      programmaticTop = root.scrollTop;
      model.input.rememberScroll?.();
      afterRestore?.();
      schedule();
    };
    measure();
  };
  const preserveUnseen = () => {
    const anchor = viewportAnchor;
    if (inputTop === null || !anchor) return;
    const moved = root.scrollTop - inputTop;
    if (Math.abs(moved) > inputReach + INPUT_SLACK_PX) return;
    anchor.offset -= moved;
    if (anchor.fallbackOffset !== undefined) anchor.fallbackOffset -= moved;
    restoreAnchor(anchor);
    inputTop = root.scrollTop;
  };
  const restoreViewportAnchor = (anchor: WindowAnchor) =>
    restoreAnchor(anchor, () => {
      if (restoringAnchor === anchor.id) restoringAnchor = null;
    });
  const anchor = model.initialAnchor;
  if (
    anchor &&
    typeof anchor.id === "string" &&
    Number.isFinite(anchor.offset)
  ) {
    const match = historyWindowAnchor(model.rows, anchor.id);
    if (anchor.kind === "message" && match) model.reveal(match.item.id);
    restoringAnchor = anchor.id;
    restoreViewportAnchor(anchor);
  } else {
    if ((model.input.followTail ?? true) && model.input.getFollow?.()) follow();
    update();
  }
  const stop = () => {
    stopRows();
    model.anchor = () => undefined;
    events.abort();
    if (frame) cancelAnimationFrame(frame);
    if (viewportRestoreFrame) cancelAnimationFrame(viewportRestoreFrame);
    observer.disconnect();
    mutations.disconnect();
  };
  const updateViewport = () => {
    const { rows, offsets, indices } = model;
    const { getFollow } = model.input;
    preserveUnseen();
    schedule();
    const offsetsChanged = previousOffsets !== offsets;
    const indicesChanged = previousIndices !== indices;
    previousIndices = indices;
    previousOffsets = offsets;
    if (previousRows !== rows) {
      const previous = previousRows;
      previousRows = rows;
      if (
        viewportAnchor &&
        (restoringAnchor === viewportAnchor.id ||
          historyRowsCanRestoreAnchor(previous, rows))
      )
        restoreViewportAnchor(viewportAnchor);
    } else if (
      (offsetsChanged || indicesChanged) &&
      viewportAnchor &&
      !getFollow?.()
    )
      restoreViewportAnchor(viewportAnchor);
  };
  const observeRows = () => {
    stopRows();
    const { active } = model;
    const restored = new WeakSet<HTMLDetailsElement>();
    const key = (detail: HTMLDetailsElement) => {
      const row = detail.closest<HTMLElement>("[data-history-row]");
      const message = detail.closest<HTMLElement>("[data-message]");
      if (detail.classList.contains("turn-work")) return null;
      if (!row && !message) return null;
      const parent = message || row!;
      return `${parent.dataset.message || row?.dataset.historyRow}:${elements(parent, "details").indexOf(detail)}:${detail.className}`;
    };
    const restore = () => {
      for (const detail of node.querySelectorAll<HTMLDetailsElement>(
        "details",
      )) {
        if (restored.has(detail)) continue;
        restored.add(detail);
        const id = key(detail);
        const open = id && model.details.get(id);
        if (typeof open === "boolean" && detail.open !== open)
          detail.open = open;
      }
    };
    const toggled = (event: Event) => {
      if (!(event.target instanceof HTMLDetailsElement)) return;
      const id = key(event.target);
      if (id) model.details.set(id, event.target.open);
    };
    node.addEventListener("toggle", toggled, true);
    restore();
    const changes = new MutationObserver(restore);
    changes.observe(node, { childList: true, subtree: true });
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const row = entry.target as HTMLElement;
        const id = row.dataset.historyRow!;
        const height =
          entry.borderBoxSize[0]?.blockSize ??
          row.getBoundingClientRect().height;
        if (model.heights.get(id) !== height) {
          model.measure(id, height);
        }
      }
    });
    if (active)
      node
        .querySelectorAll<HTMLElement>("[data-history-row]")
        .forEach((row) => observer.observe(row));
    return (stopRows = () => {
      observer.disconnect();
      changes.disconnect();
      node.removeEventListener("toggle", toggled, true);
    });
  };
  return {
    stop,
    update: updateViewport,
    observeRows,
  };
}

const elements = (root: ParentNode, selector: string) =>
  Array.from(root.querySelectorAll<HTMLElement>(selector));
