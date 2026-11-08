import type {} from "./draft-fixture-helpers.ts";
import { createDrafts } from "../../src/sync/drafts.ts";
import * as api from "../../src/api.ts";
import * as resources from "../../src/sync/resourceEvents.ts";
export async function mount() {
  window.draft = createDrafts({ api, resources });
  window.addEventListener("pagehide", window.draft.start(), { once: true });
}
