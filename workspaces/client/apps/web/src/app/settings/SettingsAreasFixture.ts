import type { ServerAreas } from "../contracts.ts";
/** Test inputs supply only the sibling capabilities exercised by that witness. */
export function settingsAreasFixture(value: unknown): ServerAreas {
  return value as ServerAreas;
}
