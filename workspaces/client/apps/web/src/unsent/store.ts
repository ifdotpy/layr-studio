// One device buffer for input that has not yet been confirmed by its server.
export const UNSENT_BUFFER_ENABLED = true;
const DATABASE = "studio-unsent-input";
const STORE = "input";

let database: Promise<IDBDatabase> | undefined;
function open() {
  return (database ??= new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  }));
}

// The array encodes server, feature, and feature-owned identity without collisions.
export function unsentKey(server: string, feature: string, id: string) {
  return JSON.stringify([server, feature, id]);
}

export function unsentStore(server: string, feature: string) {
  const prefix = (id: string) => unsentKey(server, feature, id);
  return {
    async put(id: string, value: string | Blob) {
      if (!UNSENT_BUFFER_ENABLED) return;
      const db = await open();
      await new Promise<void>((resolve, reject) => {
        const tx = db.transaction(STORE, "readwrite");
        tx.objectStore(STORE).put(value, prefix(id));
        tx.oncomplete = () => resolve();
        tx.onabort = tx.onerror = () => reject(tx.error);
      });
    },
    async delete(id: string) {
      if (!UNSENT_BUFFER_ENABLED) return;
      const db = await open();
      await new Promise<void>((resolve, reject) => {
        const tx = db.transaction(STORE, "readwrite");
        tx.objectStore(STORE).delete(prefix(id));
        tx.oncomplete = () => resolve();
        tx.onabort = tx.onerror = () => reject(tx.error);
      });
    },
    async loadAll(): Promise<[string, string | Blob][]> {
      if (!UNSENT_BUFFER_ENABLED) return [];
      const db = await open();
      return new Promise((resolve, reject) => {
        const values: [string, string | Blob][] = [];
        const tx = db.transaction(STORE, "readonly");
        const request = tx.objectStore(STORE).openCursor();
        request.onsuccess = () => {
          const cursor = request.result;
          if (!cursor) return;
          try {
            const key: unknown = JSON.parse(String(cursor.key));
            if (
              Array.isArray(key) &&
              key.length === 3 &&
              key[0] === server &&
              key[1] === feature &&
              typeof key[2] === "string" &&
              (typeof cursor.value === "string" || cursor.value instanceof Blob)
            )
              values.push([key[2], cursor.value]);
          } catch {
            /* Ignore unrelated or damaged keys. */
          }
          cursor.continue();
        };
        tx.oncomplete = () => resolve(values);
        tx.onabort = tx.onerror = () => reject(tx.error);
      });
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
