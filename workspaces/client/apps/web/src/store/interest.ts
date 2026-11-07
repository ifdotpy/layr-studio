/** Minimum continuous MobX observation before opening a resource, in milliseconds. */
export const OBSERVABLE_INTEREST_DELAY_MS = 100;

/** This helper owns render stabilization only. Releasing its reference immediately
 * marks the existing subscription store stale; that store retains transport interest
 * through its own grace interval and batches changes through resourceEvents. */
export function observeInterest(
  acquire: () => () => void,
  inactive: () => void,
) {
  let observed = false;
  let disposed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let release: (() => void) | undefined;
  const unobserved = () => {
    observed = false;
    clearTimeout(timer);
    timer = undefined;
    const stop = release;
    release = undefined;
    stop?.();
    inactive();
  };
  return {
    observed() {
      if (disposed || observed) return;
      observed = true;
      timer = setTimeout(() => {
        timer = undefined;
        if (observed && !disposed) release = acquire();
      }, OBSERVABLE_INTEREST_DELAY_MS);
    },
    unobserved,
    dispose() {
      disposed = true;
      unobserved();
    },
  };
}
