import type { DeepReadonly } from "../../types.ts";
import type { components } from "../../generated/api.ts";

export type Project = DeepReadonly<components["schemas"]["ProjectEntityDto"]>;

export type ProjectFolder = NonNullable<Project["folders"]>[number];

export function folderLabel(
  folders: readonly ProjectFolder[],
  id: string,
): string {
  const names: string[] = [];
  const seen = new Set<string>();
  let folder = folders.find((item) => item.id === id);
  while (folder && !seen.has(folder.id)) {
    seen.add(folder.id);
    names.unshift(folder.name);
    folder = folders.find((item) => item.id === folder!.parentId);
  }
  return names.join(" / ");
}
