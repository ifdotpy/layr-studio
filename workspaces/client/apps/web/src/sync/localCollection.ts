import type { EntityRow } from "./entityContracts.ts";

/** Typed in-memory projection rows. Unsent input has its own durable store. */
export type Row = EntityRow;
type Range = { gte?: string; gt?: string; lt: string };
/** Sorts after every row id; ends a range that has no upper bound. */
const AFTER_ALL_IDS = "￿";

/** A row plus the helpers that callers use on a stored document. */
export type Doc = Row & { toJSON(): Row };
export type Change = { id: string; documentData: Row; context: string };
type Listener = (change: Change) => void;
type Observer<T> =
  | ((value: T) => void)
  | { next: (value: T) => void; error?: (error: unknown) => void };

const queues = new Map<string, Promise<unknown>>();

/** This page is the only writer of its projection store, so a queue in this
 * page orders the writes. Finish old writes before a reset deletes rows. */
export async function withEntityProjectionWrite<T>(
  databaseName: string,
  write: () => Promise<T>,
): Promise<T> {
  const run = (queues.get(databaseName) ?? Promise.resolve()).then(
    write,
    write,
  );
  queues.set(
    databaseName,
    run.catch(() => {}),
  );
  return run;
}

function projectionSupersedes(previous: Row, incoming: Row) {
  if (previous.seq !== incoming.seq) return previous.seq > incoming.seq;
  if (incoming.evicted) return previous._deleted === true;
  return previous.evicted !== true;
}

function rangesOf(selector: Record<string, unknown>): Range[] {
  const one = (value: unknown): Range => {
    const id = (value as { id?: unknown } | undefined)?.id;
    if (typeof id === "string") return { gte: id, lt: `${id}\u0000` };
    const bounds = id as Record<string, unknown> | undefined;
    if (!bounds || typeof bounds !== "object")
      throw new Error("Unsupported projection query.");
    const lower = bounds.$gt === undefined ? bounds.$gte : bounds.$gt;
    if (typeof lower !== "string" && lower !== undefined)
      throw new Error("Unsupported projection query.");
    const upper = bounds.$lt === undefined ? AFTER_ALL_IDS : bounds.$lt;
    if (typeof upper !== "string")
      throw new Error("Unsupported projection query.");
    return {
      ...(bounds.$gt === undefined
        ? { gte: (lower as string | undefined) ?? "" }
        : { gt: lower as string }),
      lt: upper,
    };
  };
  if (!Object.keys(selector).length) return [{ gte: "", lt: AFTER_ALL_IDS }];
  const alternatives = selector.$or;
  return Array.isArray(alternatives) ? alternatives.map(one) : [one(selector)];
}

const inRange = (range: Range, id: string) =>
  (range.gt === undefined ? id >= (range.gte ?? "") : id > range.gt) &&
  id < range.lt;

export class LocalCollection {
  readonly database: { name: string };
  readonly $: {
    subscribe: (observer: Observer<Change>) => { unsubscribe(): void };
  };
  private readonly rows = new Map<string, Row>();
  private readonly docs = new WeakMap<Row, Doc>();
  private readonly listeners = new Set<Listener>();

  constructor(name: string) {
    this.database = { name };
    this.$ = {
      subscribe: (observer) => {
        const next = typeof observer === "function" ? observer : observer.next;
        this.listeners.add(next);
        return { unsubscribe: () => this.listeners.delete(next) };
      },
    };
  }

  async getRows(ids: string[], withDeleted: boolean) {
    return ids.flatMap((id) => {
      const row = this.rows.get(id);
      return row && (withDeleted || !row._deleted) ? [row] : [];
    });
  }

  /** The live rows of a query, ordered by id. */
  async query(selector: Record<string, unknown>, limit = Infinity) {
    const ranges = rangesOf(selector);
    return [...this.rows.values()]
      .filter((row) => !row._deleted && ranges.some((r) => inRange(r, row.id)))
      .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
      .slice(0, limit);
  }

  private doc(row: Row): Doc {
    let doc = this.docs.get(row);
    if (!doc) {
      const copy = { ...row } as Doc;
      Object.defineProperty(copy, "toJSON", { value: () => ({ ...row }) });
      doc = copy;
      this.docs.set(row, doc);
    }
    return doc;
  }

  private emit(row: Row, context: string) {
    const change = { id: row.id, documentData: row, context };
    for (const listener of this.listeners) listener(change);
  }

  /** Compare and publish one typed batch without an awaited read or a CAS retry. */
  async applyBatch(
    rows: Row[],
    {
      isCurrent = () => true,
      replace = false,
      hydrate = false,
    }: {
      isCurrent?: () => boolean;
      replace?: boolean;
      hydrate?: boolean;
    } = {},
  ): Promise<boolean> {
    if (!isCurrent()) return false;
    const added: Row[] = [];
    for (const row of rows) {
      const previous = this.rows.get(row.id);
      if (
        previous &&
        !replace &&
        (hydrate
          ? previous.seq >= row.seq
          : projectionSupersedes(previous, row))
      )
        continue;
      const stored = { ...row, _deleted: row._deleted === true };
      this.rows.set(stored.id, stored);
      added.push(stored);
    }
    for (const row of added) this.emit(row, hydrate ? "cache" : "projection");
    return true;
  }

  /** Cache decoding happens outside the interval queue. Live scope receipts win. */
  hydrate(
    rows: Row[],
    isCurrent: () => boolean = () => true,
    cursorId?: string,
  ) {
    return withEntityProjectionWrite(this.database.name, async () => {
      if (!isCurrent() || (cursorId && this.rows.has(cursorId))) return false;
      return this.applyBatch(rows, { isCurrent, hydrate: true });
    });
  }

  /** Every stored row, tombstones included. */
  all() {
    return [...this.rows.values()];
  }

  find(query: { selector?: Record<string, unknown>; limit?: number } = {}) {
    const selector = query.selector ?? {};
    const ranges = rangesOf(selector);
    const load = async () =>
      (await this.query(selector, query.limit)).map((row) => this.doc(row));
    return {
      exec: load,
      $: this.observe((id) => ranges.some((range) => inRange(range, id)), load),
    };
  }

  findOne(id: string) {
    const load = async () => {
      const [row] = await this.getRows([id], false);
      return row ? this.doc(row) : null;
    };
    return { exec: load, $: this.observe((changed) => changed === id, load) };
  }

  /** Emit the result now and after each write that touches the query. */
  private observe<T>(covers: (id: string) => boolean, load: () => Promise<T>) {
    return {
      subscribe: (
        observer: Observer<T>,
        onError?: (error: unknown) => void,
      ) => {
        const next = typeof observer === "function" ? observer : observer.next;
        const fail = typeof observer === "function" ? onError : observer.error;
        let stopped = false;
        let running = false;
        let again = false;
        const emit = async () => {
          if (running) {
            again = true;
            return;
          }
          running = true;
          try {
            do {
              again = false;
              const value = await load();
              if (!stopped) next(value);
            } while (again && !stopped);
          } catch (error) {
            if (!stopped) fail?.(error);
          } finally {
            running = false;
          }
        };
        const listener: Listener = (change) => {
          if (covers(change.id)) void emit();
        };
        this.listeners.add(listener);
        void emit();
        const subscription = {
          closed: false,
          unsubscribe: () => {
            stopped = true;
            subscription.closed = true;
            this.listeners.delete(listener);
          },
        };
        return subscription;
      },
    };
  }
}
