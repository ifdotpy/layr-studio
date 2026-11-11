import { action, onBecomeObserved, onBecomeUnobserved } from "mobx";

/** Owns observation lifetimes outside derivations, including React render. */
export function whileObserved(
  target: object,
  prop: string | undefined,
  start: () => () => void,
  abandon?: () => void,
) {
  // Undefined means there is no pending or active observation lifetime.
  let observed: boolean | undefined;
  let stop: (() => void) | undefined;
  const flush = action(() => {
    if (observed) stop ??= start();
    else if (observed === false) {
      observed = undefined;
      const release = stop ?? abandon;
      stop = undefined;
      release?.();
    }
  });
  const update = (value: boolean) => {
    if (!value && observed === undefined) return;
    observed = value;
    queueMicrotask(flush);
  };
  const observedHook = onBecomeObserved(target, prop, () => update(true));
  const unobservedHook = onBecomeUnobserved(target, prop, () => update(false));
  return () => {
    observedHook();
    unobservedHook();
    update(false);
  };
}
