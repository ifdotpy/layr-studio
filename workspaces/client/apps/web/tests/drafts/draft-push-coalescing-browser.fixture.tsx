import { readDraft } from "./draft-fixture-helpers.ts";
import { createDrafts } from "../../src/sync/drafts.ts";
import { observeOutbox } from "../../src/sync/send.ts";
import * as api from "../../src/api.ts";
import * as resources from "../../src/sync/resourceEvents.ts";
import { syncDatabase } from "../../src/sync/client.ts";
declare global {
  interface Window {
    coalescingDraft_readDraft: typeof readDraft;
  }
}
export async function mount() {
  window.coalescingDraft_readDraft = readDraft;
  window.db = (await syncDatabase()).db;
  window.draft = createDrafts({ api, resources });
  const stopDrafts = window.draft.start();
  window.outbox = { entries: [], error: "" };
  const stopOutbox = observeOutbox(
    (entries) => {
      window.outbox.entries = entries;
    },
    (error) => {
      window.outbox.error = error;
    },
  );
  window.addEventListener(
    "pagehide",
    () => {
      stopDrafts();
      stopOutbox();
    },
    { once: true },
  );
}
