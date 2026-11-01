/** Small IndexedDB adapter; each caller owns its database, store, and values. */
export function indexedDb(
  databaseName: string,
  storeName: string,
  version: number,
) {
  let database: Promise<IDBDatabase> | undefined;
  function open() {
    return (database ??= new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open(databaseName, version);
      request.onupgradeneeded = () =>
        request.result.createObjectStore(storeName);
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    }).catch((error: unknown) => {
      database = undefined;
      throw error;
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
  async function remove(key: IDBValidKey) {
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
  async function update(key: string, change: (current: unknown) => unknown) {
    const db = await open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(storeName, "readwrite");
      const store = tx.objectStore(storeName);
      const request = store.get(key);
      request.onsuccess = () => {
        const next = change(request.result);
        if (next === undefined) store.delete(key);
        else store.put(next, key);
      };
      tx.oncomplete = () => resolve();
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
  async function putMany(values: [string, unknown][]) {
    const db = await open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(storeName, "readwrite");
      const store = tx.objectStore(storeName);
      for (const [key, value] of values) store.put(value, key);
      tx.oncomplete = () => resolve();
      tx.onabort = tx.onerror = () => reject(tx.error);
    });
  }
  async function scan<T>(
    inspect: (values: [IDBValidKey, unknown][]) => {
      value: T;
      remove: IDBValidKey[];
    },
  ): Promise<T> {
    const db = await open();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(storeName, "readwrite");
      const store = tx.objectStore(storeName);
      const values: [IDBValidKey, unknown][] = [];
      let result: T;
      const request = store.openCursor();
      request.onsuccess = () => {
        const cursor = request.result;
        if (cursor) {
          values.push([cursor.key, cursor.value]);
          cursor.continue();
          return;
        }
        try {
          const inspected = inspect(values);
          result = inspected.value;
          for (const key of inspected.remove) store.delete(key);
        } catch (error) {
          tx.abort();
          reject(error);
        }
      };
      tx.oncomplete = () => resolve(result);
      tx.onabort = tx.onerror = () => reject(tx.error);
    });
  }
  return { get, put, putMany, scan, delete: remove, entries, update };
}
