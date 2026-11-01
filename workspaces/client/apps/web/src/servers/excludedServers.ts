// Servers the user removed from this UI. A removed server stays out of the
// automatic list until the user adds it again.
const removalKey = "studio-automatic-ui-removals-v1";

export function excludedServers(
  storage = globalThis.localStorage,
): Set<string> {
  const value: unknown = JSON.parse(storage.getItem(removalKey) || "[]");
  if (!Array.isArray(value) || value.some((id) => typeof id !== "string"))
    throw new Error("The saved UI removal list is invalid.");
  return new Set(value);
}

export function setServerExcluded(
  serverId: string,
  excluded: boolean,
  storage = globalThis.localStorage,
) {
  const ids = excludedServers(storage);
  if (excluded) ids.add(serverId);
  else ids.delete(serverId);
  storage.setItem(removalKey, JSON.stringify([...ids]));
}
