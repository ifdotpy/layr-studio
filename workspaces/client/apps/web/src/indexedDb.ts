/** Small IndexedDB adapter; each caller owns its database, store, and values. */
export function indexedDb(databaseName: string, storeName: string) {
  let database: Promise<IDBDatabase> | undefined;
  function open() {
    return (database ??= new Promise((resolve, reject) => {
      const request = indexedDB.open(databaseName, 1);
      request.onupgradeneeded = () =>
        request.result.createObjectStore(storeName);
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    }));
  }
  async function put(key: string, value: unknown) {
    const db = await open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(storeName, "readwrite");
      tx.objectStore(storeName).put(value, key);
      tx.oncomplete = () => resolve();
      tx.onabort = tx.onerror = () => reject(tx.error);
    });
  }
  async function remove(key: string) {
    const db = await open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(storeName, "readwrite");
      tx.objectStore(storeName).delete(key);
      tx.oncomplete = () => resolve();
      tx.onabort = tx.onerror = () => reject(tx.error);
    });
  }
  async function get(key: string): Promise<unknown> {
    const db = await open();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(storeName, "readonly");
      const request = tx.objectStore(storeName).get(key);
      tx.oncomplete = () => resolve(request.result);
      tx.onabort = tx.onerror = () => reject(tx.error);
    });
  }
  async function entries(): Promise<[IDBValidKey, unknown][]> {
    const db = await open();
    return new Promise((resolve, reject) => {
      const values: [IDBValidKey, unknown][] = [];
      const tx = db.transaction(storeName, "readonly");
      const request = tx.objectStore(storeName).openCursor();
      request.onsuccess = () => {
        const cursor = request.result;
        if (!cursor) return;
        values.push([cursor.key, cursor.value]);
        cursor.continue();
      };
      tx.oncomplete = () => resolve(values);
      tx.onabort = tx.onerror = () => reject(tx.error);
    });
  }
  return { get, put, delete: remove, entries };
}
