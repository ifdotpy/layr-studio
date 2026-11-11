import { makeAutoObservable, observableRef, compareShallow } from "mobx";
import type { RoomHistory } from "../../../../conversation/roomHistory.ts";

/** Measured row heights and the visible window belong to one room list owner. */
export class RoomViewport {
  readonly heights = new Map<string, number>();
  range = { start: 0, end: 40, top: 0, bottom: 0 };
  constructor() {
    makeAutoObservable(
      this,
      { heights: false, range: observableRef },
      { autoBind: true },
    );
  }
  measure(root: HTMLElement, items: RoomHistory["items"]) {
    const bounds = root.getBoundingClientRect();
    let seen = 0;
    for (const node of root.querySelectorAll<HTMLElement>("[data-room-row]")) {
      const rect = node.getBoundingClientRect();
      if (rect.bottom > bounds.top && rect.top < bounds.bottom)
        seen = Math.max(seen, Number(node.dataset.roomSeq) || 0);
      const height = Math.ceil(rect.height);
      if (height && node.dataset.roomRow)
        this.heights.set(node.dataset.roomRow, height);
    }
    const offsets = [0];
    for (const item of items)
      offsets.push(
        offsets.at(-1)! +
          (this.heights.get(item.id) ||
            Math.max(112, Math.ceil((item.text?.length || 0) / 78) * 23 + 82)),
      );
    const top = root.scrollTop,
      bottom = top + root.clientHeight;
    let start = 0;
    while (start < items.length && offsets[start + 1]! < top) start++;
    let end = start;
    while (end < items.length && offsets[end]! < bottom) end++;
    start = Math.max(0, start - 8);
    end = Math.min(items.length, Math.max(start + 1, end + 8));
    const next = {
      start,
      end,
      top: offsets[start]!,
      bottom: offsets.at(-1)! - offsets[end]!,
    };
    if (!compareShallow(this.range, next)) this.range = next;
    return seen;
  }
}
