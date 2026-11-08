import type { ComponentSnapshot as Snapshot } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import type { Room } from "../../src/types.ts";

// This fixture serves and reads its own snapshot and room documents.
export function radioSnapshot(value: unknown): Snapshot {
  return value as Snapshot;
}
export function radioRoom(value: unknown): Room {
  return value as Room;
}
