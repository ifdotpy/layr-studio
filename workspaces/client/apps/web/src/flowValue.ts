/** Preserve the resolved type when delegating a promise inside a MobX flow. */
export function* flowValue<T>(
  promise: Promise<T>,
): Generator<Promise<T>, T, T> {
  return yield promise;
}
