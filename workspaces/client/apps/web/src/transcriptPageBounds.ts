import type { Message } from "./types";
import { messageRenderKey } from "./components/message-delivery/messageDelivery";
import { isMessagePending } from "./messageSendStatus.ts";

export const MAX_TRANSCRIPT_PAGE_ITEMS = 240;
export const MAX_TRANSCRIPT_PAGE_BYTES = 4_000_000;
export const MAX_TRANSCRIPT_CACHE_ITEMS = 1_200;
export const MAX_TRANSCRIPT_CACHE_BYTES = 12_000_000;

export function mergeTranscript(
  earlier: Message[],
  later: Message[],
): Message[] {
  return [
    ...new Map([...earlier, ...later].map((item) => [item.id, item])).values(),
  ].sort(
    (a, b) => Number(a.at ?? a.created ?? 0) - Number(b.at ?? b.created ?? 0),
  );
}

export function updateLiveTranscriptPage(
  page: {
    items: Message[];
    before: string | null | undefined;
    focused: boolean;
  },
  liveItems: Message[],
  snapshot: {
    truncated?: boolean | undefined;
    nextCursor?: string | null | undefined;
  },
) {
  if (page.focused) {
    const present = new Set(page.items.map((item) => item.id));
    page.items = mergeTranscript(
      page.items,
      liveItems.filter((item) => present.has(item.id)),
    );
  } else if (!snapshot.truncated) {
    page.items = liveItems;
    page.before = null;
  } else {
    const present = new Set(liveItems.map((item) => item.id));
    // Only an authoritative shared row proves that the retained and fresh
    // ranges are contiguous. Optimistic queue rows can span a history gap.
    const authoritative = new Set(
      liveItems
        .filter(
          (item) =>
            !isMessagePending(item) &&
            !item.streamPending &&
            item.materialized !== false,
        )
        .map((item) => item.id),
    );
    const overlaps = page.items.some(
      (item) =>
        !isMessagePending(item) &&
        !item.streamPending &&
        item.materialized !== false &&
        authoritative.has(item.id),
    );
    if (!overlaps) {
      page.items = liveItems;
      page.before = snapshot.nextCursor || liveItems[0]?.id || null;
      return;
    }
    // A live snapshot is authoritative within its range, including removals.
    const firstAt = Math.min(
      ...liveItems.map((item) => Number(item.at ?? item.created ?? Infinity)),
    );
    page.items = mergeTranscript(
      page.items.filter(
        (item) =>
          present.has(item.id) ||
          (!isMessagePending(item) &&
            !item.streamPending &&
            item.materialized !== false &&
            Number(item.at ?? item.created ?? 0) < firstAt),
      ),
      liveItems,
    );
  }
}

export function boundTranscriptPage(
  items: Message[],
  sizeOf: (item: Message) => number,
  direction: "older" | "newer" | "around" | "latest",
  cursors: {
    before: string | null;
    after: string | null;
    nextBefore: string | null;
    nextAfter: string | null;
  },
  anchorId?: string,
  visibleEndId?: string,
) {
  const bounded = boundTranscriptItems(
    items,
    sizeOf,
    direction === "older" ? "oldest" : "newest",
    anchorId,
    direction === "around",
    visibleEndId,
  );
  let before = direction === "newer" ? cursors.before : cursors.nextBefore;
  let after = cursors.nextAfter;
  if (bounded.droppedOldest) before = bounded.items[0]?.id || before;
  if (direction === "older")
    after =
      bounded.droppedNewest || cursors.after
        ? bounded.items.at(-1)?.id || null
        : null;
  else if (direction === "newer") {
    if (bounded.droppedNewest || cursors.nextAfter)
      after = bounded.items.at(-1)?.id || null;
    before =
      bounded.droppedOldest || cursors.before
        ? bounded.items[0]?.id || null
        : null;
  } else if (direction === "around" && bounded.droppedNewest)
    after = bounded.items.at(-1)?.id || null;
  return { ...bounded, before, after };
}

export function boundTranscriptItems(
  items: Message[],
  sizeOf: (item: Message) => number,
  keep: "oldest" | "newest" = "newest",
  anchorId?: string,
  centered = false,
  visibleEndId?: string,
) {
  const result: Message[] = [];
  let bytes = 0;
  const anchorIndex = anchorId
    ? items.findIndex(
        (item) =>
          messageRenderKey(item) === anchorId ||
          item.id === anchorId ||
          item.sourceId === anchorId,
      )
    : -1;
  if (anchorIndex >= 0) {
    let first = anchorIndex;
    let last = anchorIndex;
    result.push(items[anchorIndex]!);
    bytes = sizeOf(items[anchorIndex]!);
    const extend = (step: -1 | 1, limit = MAX_TRANSCRIPT_PAGE_ITEMS) => {
      while (result.length < limit) {
        const index = step < 0 ? first - 1 : last + 1;
        const item = items[index];
        if (!item || bytes + sizeOf(item) > MAX_TRANSCRIPT_PAGE_BYTES) break;
        if (step < 0) {
          result.unshift(item);
          first = index;
        } else {
          result.push(item);
          last = index;
        }
        bytes += sizeOf(item);
      }
    };
    if (visibleEndId && keep === "oldest") {
      const end = items.findIndex(
        (item) => item.id === visibleEndId || item.sourceId === visibleEndId,
      );
      extend(1, Math.max(1, end - anchorIndex + 1));
    }
    if (centered) extend(-1, 121);
    extend(keep === "oldest" ? -1 : 1);
    extend(keep === "oldest" ? 1 : -1);
  } else {
    const source = keep === "oldest" ? items : [...items].reverse();
    for (const item of source) {
      const size = sizeOf(item);
      if (
        result.length >= MAX_TRANSCRIPT_PAGE_ITEMS ||
        bytes + size > MAX_TRANSCRIPT_PAGE_BYTES
      )
        break;
      result.push(item);
      bytes += size;
    }
    if (keep === "newest") result.reverse();
  }
  const retained = new Set(result);
  const firstRetained = items.findIndex((item) => retained.has(item));
  return {
    items: result,
    bytes,
    droppedOldest: Math.max(0, firstRetained),
    droppedNewest: Math.max(
      0,
      items.length - (Math.max(0, firstRetained) + result.length),
    ),
  };
}

export function trimTranscriptPageCache<
  T extends { items: Message[]; size: number },
>(pages: Map<string, T>, keepScope: string) {
  let items = 0;
  let bytes = 0;
  for (const page of pages.values()) {
    items += page.items.length;
    bytes += page.size;
  }
  while (
    pages.size > 12 ||
    items > MAX_TRANSCRIPT_CACHE_ITEMS ||
    bytes > MAX_TRANSCRIPT_CACHE_BYTES
  ) {
    const oldest = [...pages.keys()].find((scope) => scope !== keepScope);
    if (!oldest) break;
    const page = pages.get(oldest)!;
    pages.delete(oldest);
    items -= page.items.length;
    bytes -= page.size;
  }
}
