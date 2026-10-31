import { ApiError } from "../api.ts";
import { serverMutationResponseIsUncertain } from "../servers/mutationIdentity.ts";
import type { components } from "../generated/api.ts";
import { resolveDraftConflict } from "./conflicts.ts";

type Document = components["schemas"]["SyncDocument"];
type PushRow = components["schemas"]["DraftPushRow"];

const input = (document: Document) => ({
  id: document.id,
  payload: document.payload,
  seq: document.seq,
  deleted: document._deleted,
});

/** One tab's server rows and unsent edits. */
export function createDirectDrafts(options: {
  post: (rows: PushRow[]) => Promise<Document[]>;
  changed: () => void;
  buffer?: {
    put: (key: string, value: string) => Promise<void>;
    delete: (key: string) => Promise<void>;
    loadAll: () => Promise<[string, string | Blob][]>;
  };
}) {
  const masters = new Map<string, Document>();
  const pending = new Map<string, Document>();
  const writes = new Map<string, Promise<void>>();
  let sequence = -1;
  let flushing: Promise<void> | undefined;

  const apply = (rows: Document[]) => {
    for (const row of rows) {
      const previous = masters.get(row.id);
      if (!previous || row.seq >= previous.seq) masters.set(row.id, row);
    }
    options.changed();
  };
  const flush = (): Promise<void> => {
    if (sequence < 0) return Promise.resolve();
    if (flushing) return flushing;
    flushing = (async () => {
      while (pending.size && sequence >= 0) {
        const sent = [...pending.values()].slice(0, 1);
        const rows = sent.map((document) => {
          const master = masters.get(document.id);
          return {
            newDocumentState: input(document),
            ...(master ? { assumedMasterState: input(master) } : {}),
          };
        });
        const document = sent[0]!;
        const write = writes.get(document.id);
        if (write) await write;
        let conflicts: Document[];
        try {
          conflicts = await options.post(rows);
        } catch (error) {
          if (
            error instanceof ApiError &&
            !serverMutationResponseIsUncertain(error.status, error.response) &&
            pending.get(document.id) === document
          ) {
            pending.delete(document.id);
            await options.buffer?.delete(document.id);
          }
          throw error;
        }
        const rejected = new Map(conflicts.map((row) => [row.id, row]));
        for (const [index, document] of sent.entries()) {
          const master = rejected.get(document.id);
          if (master) {
            masters.set(master.id, master);
            const local = pending.get(document.id);
            if (local) {
              const resolved = await resolveDraftConflict(master, local);
              pending.set(local.id, resolved);
              if (options.buffer)
                await options.buffer.put(local.id, JSON.stringify(resolved));
            }
          } else {
            const existing = masters.get(document.id);
            const assumed = rows[index]?.assumedMasterState;
            if (
              !existing ||
              (assumed &&
                existing.payload === assumed.payload &&
                existing._deleted === assumed.deleted)
            )
              masters.set(document.id, {
                ...document,
                seq: existing?.seq ?? 0,
              });
            if (pending.get(document.id) === document) {
              pending.delete(document.id);
              await options.buffer?.delete(document.id);
            }
          }
        }
        options.changed();
      }
    })().finally(() => {
      flushing = undefined;
    });
    return flushing;
  };
  return {
    snapshot(rows: Document[], _complete: boolean, revision: number) {
      masters.clear();
      sequence = revision;
      apply(rows);
    },
    changes(rows: Document[], revision: number) {
      sequence = revision;
      apply(rows);
    },
    sequence: () => sequence,
    waitForBaseline() {
      sequence = -1;
    },
    edit(row: Document) {
      pending.set(row.id, row);
      if (options.buffer) {
        const write = (
          writes.get(row.id)?.catch(() => {}) ?? Promise.resolve()
        ).then(() => options.buffer!.put(row.id, JSON.stringify(row)));
        writes.set(row.id, write);
        void write.catch(() => {});
      }
    },
    async restore() {
      for (const [id, value] of (await options.buffer?.loadAll()) ?? []) {
        if (typeof value !== "string" || pending.has(id)) continue;
        const row: unknown = JSON.parse(value);
        if (
          row &&
          typeof row === "object" &&
          "id" in row &&
          row.id === id &&
          "payload" in row &&
          typeof row.payload === "string" &&
          "seq" in row &&
          typeof row.seq === "number" &&
          "_deleted" in row &&
          typeof row._deleted === "boolean"
        )
          pending.set(id, row as Document);
      }
      options.changed();
    },
    records: () =>
      [...new Map([...masters, ...pending]).values()].filter(
        (row) => !row._deleted,
      ),
    flush,
  };
}
