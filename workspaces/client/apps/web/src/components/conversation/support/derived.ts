import { compareShallow, computed, type IComputedValueOptions } from "mobx";

/** Keep a model's keyed derivation stable without mirroring its caller's props. */
export function derived<A extends unknown[], T>(
  read: (...args: A) => T,
  options?: IComputedValueOptions<T>,
  equalsArgs: (previous: A, next: A) => boolean = compareShallow,
) {
  let previous: A | undefined;
  let value: ReturnType<typeof computed<T>>;
  return (...args: A): T => {
    if (!previous || !equalsArgs(previous, args)) {
      previous = args;
      value = computed(() => read(...args), options);
    }
    return value.get();
  };
}
