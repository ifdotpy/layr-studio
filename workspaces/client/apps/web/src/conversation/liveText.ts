import { createContext, useContext } from "react";
import { observable, runInAction } from "mobx";

/** Text of one item that the server sent live, before the durable item holds it. `start` and
 * `length` count Unicode scalars, as the server does. */
export type LiveText = { start: number; length: number; text: string };

/** Items kept per chat; older ones are long durable. */
export const MAX_LIVE_ITEMS = 8;

/** The durable text with the live run on top of it. The durable text wins when it is at least as
 * long, or when it is too short to join the run. */
export function withLiveText(durable: string, live: LiveText | undefined) {
  if (!live) return durable;
  const scalars = Array.from(durable);
  if (scalars.length < live.start || scalars.length >= live.start + live.length)
    return durable;
  return scalars.slice(0, live.start).join("") + live.text;
}

/** One ephemeral owner for the open history; each leaf observes only its key. */
export function createLiveText() {
  const runs = observable.map<string, LiveText>(undefined, { deep: false });
  return {
    get: (id: string) => runs.get(id),
    add(delta: { itemId: string; offset: number; text: string }) {
      runInAction(() => {
        const run = runs.get(delta.itemId);
        const length = Array.from(delta.text).length;
        runs.delete(delta.itemId);
        runs.set(
          delta.itemId,
          run && run.start + run.length === delta.offset
            ? {
                start: run.start,
                length: run.length + length,
                text: run.text + delta.text,
              }
            : { start: delta.offset, length, text: delta.text },
        );
        while (runs.size > MAX_LIVE_ITEMS)
          runs.delete(runs.keys().next().value!);
      });
    },
    remove(id: string) {
      runInAction(() => runs.delete(id));
    },
    clear() {
      runInAction(() => runs.clear());
    },
  };
}
export const HistoryLiveText = createContext<ReturnType<
  typeof createLiveText
> | null>(null);
export function useItemText(
  item: { id: string; text: string } | undefined,
  fallback: string,
) {
  const live = useContext(HistoryLiveText);
  return item ? withLiveText(item.text, live?.get(item.id)) : fallback;
}
