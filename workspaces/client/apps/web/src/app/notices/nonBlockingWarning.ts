import type { Message } from "../../types.ts";

export function isNonBlockingWarning(item: Message): boolean {
  return (
    "nativeNotice" in item &&
    item.nativeNotice === "warning" &&
    !("nativeError" in item && item.nativeError) &&
    !("nativeReview" in item && item.nativeReview) &&
    !("nativeHook" in item && item.nativeHook) &&
    !("nativeHookQuiet" in item && item.nativeHookQuiet)
  );
}
