import type { components } from "../../src/generated/api.ts";

/** Empty radio state for sidebar fixtures that only render the shared chat row. */
export const sidebarFixtureRadio = {
  controls: {
    busy: false,
    inputBlocked: false,
    canPass: false,
    stopping: false,
    speaking: false,
    busyNotice: null,
  },
  direct: true,
  active: null,
  error: null,
  next: [],
  revision: 0,
  speaker: null,
  status: "idle",
  teamId: "",
} satisfies components["schemas"]["RoomRadio"];
