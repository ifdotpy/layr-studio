import { RoomKind } from "./generated/api";
import { expect, it } from "vitest";
import { roomLeadIds } from "./chatScope";
import type { Room } from "./types";

it("uses the server's room roots", () => {
  const room = {
    kind: RoomKind.private,
    members: ["a"],
    leadIds: ["lead"],
  } as Room;
  expect(roomLeadIds(room)).toEqual(["lead"]);
});
