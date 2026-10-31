import { indexedDb } from "../indexedDb.ts";

// One device buffer for input that has not yet been confirmed by its server.
export const UNSENT_BUFFER_ENABLED = true;
export const UNSENT_FAILURE_EVENT = "studio-unsent-input-failure";
export function reportUnsentFailure() {
  window.dispatchEvent(new Event(UNSENT_FAILURE_EVENT));
}
const DATABASE = "studio-unsent-input";
const STORE = "input";

const db = indexedDb(DATABASE, STORE);

// The array encodes server, feature, and feature-owned identity without collisions.
export function unsentKey(server: string, feature: string, id: string) {
  return JSON.stringify([server, feature, id]);
}

export function unsentStore(server: string, feature: string) {
  const prefix = (id: string) => unsentKey(server, feature, id);
  return {
    async put(id: string, value: string | Blob) {
      if (UNSENT_BUFFER_ENABLED) await db.put(prefix(id), value);
    },
    async delete(id: string) {
      if (UNSENT_BUFFER_ENABLED) await db.delete(prefix(id));
    },
    async loadAll(): Promise<[string, string | Blob][]> {
      if (!UNSENT_BUFFER_ENABLED) return [];
      const values: [string, string | Blob][] = [];
      for (const [storedKey, value] of await db.entries()) {
        try {
          const key: unknown = JSON.parse(String(storedKey));
          if (
            Array.isArray(key) &&
            key.length === 3 &&
            key[0] === server &&
            key[1] === feature &&
            typeof key[2] === "string" &&
            (typeof value === "string" || value instanceof Blob)
          )
            values.push([key[2], value]);
        } catch {
          /* Ignore unrelated or damaged keys. */
        }
      }
      return values;
    },
  };
}

// Remove this localStorage migration after release 0.2.0 has shipped.
export async function migrateMessageSends(
  storage: Storage,
  buffer: ReturnType<typeof unsentStore>,
) {
  if (!UNSENT_BUFFER_ENABLED) return;
  const prefix = "studio-message-send:";
  const existing = new Set((await buffer.loadAll()).map(([key]) => key));
  const keys = Array.from({ length: storage.length }, (_, index) =>
    storage.key(index),
  );
  for (const key of keys) {
    if (!key?.startsWith(prefix)) continue;
    const value = storage.getItem(key);
    if (value === null) continue;
    const id = key.slice(prefix.length);
    if (!existing.has(id)) await buffer.put(id, value);
    storage.removeItem(key);
  }
}
