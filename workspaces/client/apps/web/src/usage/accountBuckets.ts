import type { components } from "../generated/api";

type BucketProjection = components["schemas"]["BucketProjection"];

/** Select the server's aggregate state at its supplied deadline. */
export function accountBuckets(
  view: BucketProjection | null | undefined,
  now: number,
) {
  if (!view) return null;
  return view.validUntil !== null && now >= view.validUntil
    ? view.next
    : view.current;
}
