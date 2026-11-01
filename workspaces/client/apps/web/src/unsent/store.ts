import { indexedDb } from "../indexedDb.ts";
import { isUnsentRecord, UNSENT_FORMAT_VERSION } from "./schema.ts";
import type {
  UnsentRecord,
  RecordingValue,
  StoredSend,
  StoredUpload,
} from "./schema.ts";
export type { RecordingValue, StoredSend, StoredUpload } from "./schema.ts";

// One device buffer for input that has not yet been confirmed by its server.
export const UNSENT_FAILURE_EVENT = "studio-unsent-input-failure";
export function reportUnsentFailure() {
  if (
    typeof window !== "undefined" &&
    typeof window.dispatchEvent === "function"
  )
    window.dispatchEvent(new Event(UNSENT_FAILURE_EVENT));
}
// IndexedDB database name for durable, unconfirmed device input.
const DATABASE = "studio-unsent-input";
// IndexedDB schema version, measured in schema revisions.
const DATABASE_VERSION = 1;
// Object store name for all unconfirmed device input.
const STORE = "input";

const db = indexedDb(DATABASE, STORE, DATABASE_VERSION);

type StoredValue = Extract<UnsentRecord, { kind: "text" }>;
export type TextFeature = "form" | "answer" | "queue-edit" | "reply";
function putRecord(key: string, value: UnsentRecord) {
  return db.put(key, value);
}
function encode(value: string): StoredValue {
  return { version: UNSENT_FORMAT_VERSION, kind: "text", value };
}
function decode(value: unknown): StoredValue | undefined {
  return isUnsentRecord(value) && value.kind === "text" ? value : undefined;
}

// The array encodes server, feature, and feature-owned identity without collisions.
export function unsentKey(server: string, feature: string, id: string) {
  return JSON.stringify([server, feature, id]);
}

export function unsentStore(server: string, feature: TextFeature) {
  const prefix = (id: string) => unsentKey(server, feature, id);
  return {
    async put(id: string, value: string) {
      await putRecord(prefix(id), encode(value));
    },
    async delete(id: string) {
      await db.delete(prefix(id));
    },
    async deleteIfEqual(id: string, value: string) {
      await db.update(prefix(id), (current) =>
        decode(current)?.value === value ? undefined : current,
      );
    },
    async loadAll(): Promise<[string, string][]> {
      return (await loadRecords(server, feature, decode)).map(
        ([id, record]) => [id, record.value],
      );
    },
  };
}

function decodeKey(key: IDBValidKey): [string, string, string] | undefined {
  if (typeof key !== "string") return;
  try {
    const parts: unknown = JSON.parse(key);
    if (
      Array.isArray(parts) &&
      parts.length === 3 &&
      typeof parts[0] === "string" &&
      typeof parts[1] === "string" &&
      typeof parts[2] === "string" &&
      [
        "form",
        "answer",
        "queue-edit",
        "reply",
        "draft",
        "upload",
        "recording",
        "message",
      ].includes(parts[1])
    )
      return [parts[0], parts[1], parts[2]];
  } catch {
    /* The key is invalid. */
  }
  return undefined;
}

async function loadRecords<T>(
  server: string,
  feature: string,
  decodeRecord: (value: unknown, id: string) => T | undefined,
): Promise<[string, T][]> {
  const values: [string, T][] = [];
  for (const [storedKey, value] of await db.entries()) {
    const identity = decodeKey(storedKey);
    if (!identity) {
      console.error("Discarding invalid unsent input key", storedKey);
      await db.delete(storedKey);
      continue;
    }
    if (identity[0] !== server || identity[1] !== feature) continue;
    const id = identity[2];
    const row = decodeRecord(value, id);
    if (row !== undefined) values.push([id, row]);
    else {
      console.error("Discarding invalid unsent input", storedKey);
      await db.update(String(storedKey), (current) =>
        decodeRecord(current, id) === undefined ? undefined : current,
      );
    }
  }
  return values;
}

type Draft =
  import("../generated/api.ts").components["schemas"]["SyncDocument"];

function decodeDraft(value: unknown, id: string): Draft | undefined {
  return isUnsentRecord(value) &&
    value.kind === "draft" &&
    value.value.id === id
    ? value.value
    : undefined;
}

export function draftStore(server: string) {
  const key = (id: string) => unsentKey(server, "draft", id);
  return {
    async put(id: string, value: Draft) {
      await putRecord(key(id), {
        version: UNSENT_FORMAT_VERSION,
        kind: "draft",
        value,
      });
    },
    async delete(id: string) {
      await db.delete(key(id));
    },
    loadAll: () => loadRecords(server, "draft", decodeDraft),
  };
}

function decodeUpload(value: unknown, id: string): StoredUpload | undefined {
  return isUnsentRecord(value) &&
    value.kind === "upload" &&
    value.value.id === id
    ? value.value
    : undefined;
}
export function uploadStore(server: string) {
  const key = (id: string) => unsentKey(server, "upload", id);
  return {
    async put(id: string, value: StoredUpload) {
      await putRecord(key(id), {
        version: UNSENT_FORMAT_VERSION,
        kind: "upload",
        value,
      });
    },
    async delete(id: string) {
      await db.delete(key(id));
    },
    loadAll: () => loadRecords(server, "upload", decodeUpload),
  };
}

function decodeRecording(raw: unknown): RecordingValue | undefined {
  if (!isUnsentRecord(raw)) return;
  if (raw.kind === "record") return { kind: "record", value: raw.value };
  if (raw.kind === "chunk")
    return { kind: "chunk", id: raw.id, index: raw.index, value: raw.value };
  return undefined;
}
export function recordingStore(server: string) {
  const key = (id: string) => unsentKey(server, "recording", id);
  return {
    async put(id: string, value: RecordingValue) {
      await putRecord(key(id), { version: UNSENT_FORMAT_VERSION, ...value });
    },
    async delete(id: string) {
      await db.delete(key(id));
    },
    async putChunk(
      id: string,
      record: RecordingValue,
      chunkId: string,
      chunk: RecordingValue,
    ) {
      await db.putMany([
        [key(id), { version: UNSENT_FORMAT_VERSION, ...record }],
        [key(chunkId), { version: UNSENT_FORMAT_VERSION, ...chunk }],
      ]);
    },
    loadAll: () =>
      db.scan((entries) => {
        const groups = new Map<
          string,
          { entries: [IDBValidKey, string, RecordingValue | undefined][] }
        >();
        const remove: IDBValidKey[] = [];
        for (const [storedKey, raw] of entries) {
          const identity = decodeKey(storedKey);
          if (!identity) {
            console.error("Discarding invalid unsent input key", storedKey);
            remove.push(storedKey);
            continue;
          }
          if (identity[0] !== server || identity[1] !== "recording") continue;
          const suffix = identity[2];
          let id: string | undefined;
          if (suffix.startsWith("record:")) id = suffix.slice(7);
          else if (suffix.startsWith("chunk:")) {
            try {
              const parts: unknown = JSON.parse(suffix.slice(6));
              if (
                Array.isArray(parts) &&
                parts.length === 2 &&
                typeof parts[0] === "string"
              )
                id = parts[0];
            } catch {
              /* Invalid identity is discarded below. */
            }
          }
          if (id === undefined) {
            remove.push(storedKey);
            continue;
          }
          const row = decodeRecording(raw);
          const valid =
            row &&
            (row.kind === "record"
              ? suffix === `record:${row.value.id}`
              : suffix === `chunk:${JSON.stringify([row.id, row.index])}`)
              ? row
              : undefined;
          const group = groups.get(id) ?? { entries: [] };
          group.entries.push([storedKey, suffix, valid]);
          groups.set(id, group);
        }
        const value: [string, RecordingValue][] = [];
        for (const [id, group] of groups) {
          const header = group.entries.find(
            ([, , row]) => row?.kind === "record",
          )?.[2];
          const chunks = group.entries
            .flatMap(([, , row]) => (row?.kind === "chunk" ? [row] : []))
            .sort((a, b) => a.index - b.index);
          if (
            !header ||
            header.kind !== "record" ||
            group.entries.some(([, , row]) => !row) ||
            chunks.some((chunk, index) => chunk.index !== index) ||
            chunks.reduce((total, chunk) => total + chunk.value.size / 2, 0) !==
              header.value.samples
          ) {
            console.error("Discarding invalid recording", id);
            remove.push(...group.entries.map(([storedKey]) => storedKey));
          } else {
            for (const [, suffix, row] of group.entries)
              if (row) value.push([suffix, row]);
          }
        }
        return { value, remove };
      }),
  };
}

function decodeSend(raw: unknown, id: string): StoredSend | undefined {
  if (!isUnsentRecord(raw) || raw.kind !== "send") return;
  return JSON.stringify([raw.value.request.target, raw.value.request.id]) === id
    ? raw.value
    : undefined;
}
export function sendStore(server: string) {
  const key = (id: string) => unsentKey(server, "message", id);
  const wrap = (value: StoredSend): UnsentRecord => ({
    version: UNSENT_FORMAT_VERSION,
    kind: "send",
    value,
  });
  return {
    async put(id: string, value: StoredSend) {
      await putRecord(key(id), wrap(value));
    },
    async putIfExists(id: string, value: StoredSend) {
      await db.update(key(id), (current) =>
        current === undefined ? undefined : wrap(value),
      );
    },
    async delete(id: string) {
      await db.delete(key(id));
    },
    loadAll: () => loadRecords(server, "message", decodeSend),
  };
}
