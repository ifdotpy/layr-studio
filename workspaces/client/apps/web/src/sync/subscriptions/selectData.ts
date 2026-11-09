import type { ResourceData, ResourceRef } from "./contracts.ts";

type Ref<K extends ResourceRef["kind"]> = Extract<ResourceRef, { kind: K }>;

/** Each selector is checked against the generated resource-to-data owner. */
export type ResourceDataSelectors<Input> = {
  [K in ResourceRef["kind"]]?: (
    ref: Ref<K>,
    input: Input,
  ) => ResourceData<Ref<K>> | undefined;
};

/** TypeScript loses the correlation when indexing a heterogeneous function map.
 * The key and argument come from the same ref; the map checks every result. */
export function selectResourceData<R extends ResourceRef, Input>(
  selectors: ResourceDataSelectors<Input>,
  ref: R,
  input: Input,
): ResourceData<R> | undefined {
  const select = selectors[ref.kind] as
    | ((ref: R, input: Input) => ResourceData<R> | undefined)
    | undefined;
  return select?.(ref, input);
}
