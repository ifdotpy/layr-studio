// Wire optional fields allow explicit undefined in TypeScript. Native SDK
// optional properties require those members to be absent instead.
export type DefinedFields<T> = T extends readonly unknown[]
  ? { [K in keyof T]: DefinedFields<T[K]> }
  : T extends object
    ? { [K in keyof T]: DefinedFields<Exclude<T[K], undefined>> }
    : T;
export function definedFields<T>(value: T): DefinedFields<T> {
  if (Array.isArray(value))
    return value.map((entry: unknown) =>
      definedFields(entry),
    ) as DefinedFields<T>;
  if (value !== null && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value)
        .filter(([, entry]) => entry !== undefined)
        .map(([key, entry]: [string, unknown]) => [key, definedFields(entry)]),
    ) as DefinedFields<T>;
  return value as DefinedFields<T>;
}
