import { createAtom } from "mobx";

/**
 * The single observable wall clock of the web client, in epoch milliseconds.
 * It ticks once per second only while some observer reads it, so countdowns
 * and relative times are plain `computed` values, not effects with timers.
 */
const TICK_MS = 1000;
let timer: ReturnType<typeof setInterval> | undefined;
const atom = createAtom(
  "clock",
  () => {
    timer = setInterval(() => atom.reportChanged(), TICK_MS);
  },
  () => {
    clearInterval(timer);
    timer = undefined;
  },
);

/** The current time; reading it inside a reaction or observer subscribes to ticks. */
export function now(): number {
  atom.reportObserved();
  return Date.now();
}

/** One atom per pending deadline (epoch ms); it exists only while observed. */
const deadlines = new Map<number, ReturnType<typeof createAtom>>();

/**
 * Whether the epoch-millisecond deadline has passed. An observer is notified
 * exactly at the deadline, independent of the one-second `now()` ticks.
 */
export function reached(at: number): boolean {
  if (Date.now() >= at) return true;
  let deadline = deadlines.get(at);
  if (!deadline) {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const created = createAtom(
      "clock deadline",
      () => {
        timer = setTimeout(() => created.reportChanged(), at - Date.now());
      },
      () => {
        clearTimeout(timer);
        deadlines.delete(at);
      },
    );
    deadlines.set(at, (deadline = created));
  }
  deadline.reportObserved();
  return Date.now() >= at;
}
