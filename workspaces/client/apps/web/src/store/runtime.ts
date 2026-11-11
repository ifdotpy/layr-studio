import {
  action,
  createAtom,
  flow,
  onBecomeObserved,
  onBecomeUnobserved,
} from "mobx";
import type { DeepReadonly } from "../deepReadonly.ts";
import type {
  ResourceData,
  ResourceRef,
  ResourceVersion,
  SubscriptionError,
} from "../sync/subscriptions/contracts.ts";
import { subscriptionKey } from "../sync/subscriptions/contracts.ts";
import type { createSubscriptions } from "../sync/subscriptions/module.ts";
import { observeInterest } from "./interest.ts";

export type ResourceStatus<T> =
  | { readonly status: "loading" }
  | { readonly status: "absent" }
  | { readonly status: "error"; readonly error: SubscriptionError }
  | { readonly status: "live"; readonly data: T }
  | {
      readonly status: "stale";
      readonly data: T;
      readonly error?: SubscriptionError;
    };

export function resourceData<T>(current: ResourceStatus<T>): T | undefined {
  return current.status === "live" || current.status === "stale"
    ? current.data
    : undefined;
}

export interface ResourceSlot<T> {
  readonly current: ResourceStatus<T>;
  readonly appliedVersion?: Readonly<ResourceVersion> | undefined;
}
export interface ReadIntent<T> {
  key: string;
  read(): Promise<T>;
}
export interface ReadSlot<T> extends ResourceSlot<T> {
  read(): Promise<T>;
}

/** Maximum cached unobserved keyed facades per server runtime, in entries. */
export const MAX_INACTIVE_RESOURCE_FACADES = 128;

export interface ResourceDefinition<R extends ResourceRef, TData> {
  create(dto: DeepReadonly<ResourceData<R>>): TData;
  /** Mutable models apply in place; scalar and union roots may be replaced. */
  apply(data: TData, dto: DeepReadonly<ResourceData<R>>): TData | void;
}

export interface ObservableResourceRuntime {
  slot<R extends ResourceRef, TData>(
    definition: ResourceDefinition<R, TData> & {
      key: string;
      ref: R;
    },
  ): ResourceSlot<DeepReadonly<TData>>;
  keyed<R extends ResourceRef, TData>(
    definition: ResourceDefinition<R, TData>,
  ): (ref: R) => ResourceSlot<DeepReadonly<TData>>;
}

type Subscriptions = Pick<
  ReturnType<typeof createSubscriptions>,
  "subscribe" | "getSnapshot" | "onEvict"
>;

/** One runtime belongs to one server. Generated facades retain typed slot
 * identities; this map retains only their lifecycle, never erased model values. */
export function createObservableResourceRuntime(subscriptions: Subscriptions) {
  type Entry = {
    resourceKey: string;
    observed: boolean;
    keyed: boolean;
    isReading?(): boolean;
    dispose(): void;
    evict(): void;
  };
  const entries = new Map<string, Entry>();
  const touch = (key: string) => {
    const entry = entries.get(key);
    if (entry) {
      entries.delete(key);
      entries.set(key, entry);
    }
  };
  const trim = () => {
    const inactive = [...entries.values()].filter(
      (entry) => entry.keyed && !entry.observed && !entry.isReading?.(),
    );
    for (const entry of inactive.slice(
      0,
      Math.max(0, inactive.length - MAX_INACTIVE_RESOURCE_FACADES),
    ))
      entry.evict();
  };
  const pending = new Set<() => void>();
  let disposed = false;
  const stopEvictions = subscriptions.onEvict((ref) => {
    const resourceKey = subscriptionKey(ref);
    // Notification can touch/reinsert observed keyed entries; collect before eviction.
    const projections = [...entries.values()].filter(
      (entry) => entry.resourceKey === resourceKey,
    );
    for (const entry of projections) entry.evict();
  });
  const schedule = (apply: () => void) => {
    const scheduled = pending.size > 0;
    pending.add(apply);
    if (scheduled) return;
    queueMicrotask(
      action(() => {
        const batch = [...pending];
        pending.clear();
        if (!disposed) batch.forEach((apply) => apply());
      }),
    );
  };

  function slot<R extends ResourceRef, TData>(
    definition: ResourceDefinition<R, TData> & { key: string; ref: R },
    remove?: () => void,
  ): ReadSlot<DeepReadonly<TData>>;
  function slot<TData>(
    definition: ReadIntent<TData>,
    remove?: () => void,
    releaseOnUnobserved?: boolean,
  ): ReadSlot<TData>;
  function slot<R extends ResourceRef, TData>(
    definition:
      | (ResourceDefinition<R, TData> & { key: string; ref: R })
      | ReadIntent<TData>,
    remove?: () => void,
    releaseOnUnobserved = false,
  ) {
    const { key } = definition;
    const ref = "ref" in definition ? definition.ref : undefined;
    let request: Promise<TData> | undefined,
      generation = 0;
    let releaseAfterRead = false;
    if (disposed) throw new Error("The observable resource store is closed.");
    if (entries.has(key)) throw new Error(`Duplicate resource slot: ${key}`);
    let data: TData | undefined;
    let appliedVersion: Readonly<ResourceVersion> | undefined;
    let current: ResourceStatus<TData | DeepReadonly<TData>> = {
      status: ref ? "loading" : "absent",
    };
    const update = () => {
      if (!("ref" in definition)) return;
      const { ref, create, apply } = definition;
      const snapshot = subscriptions.getSnapshot(ref);
      if (snapshot.data !== undefined) {
        // Readonly views change no representation. Ref-to-DTO correlation is
        // checked by the generated facade through ResourceData<R>.
        const dto = snapshot.data as DeepReadonly<typeof snapshot.data>;
        if (data === undefined) data = create(dto);
        else {
          const replacement = apply(data, dto);
          if (replacement !== undefined) data = replacement;
        }
      }
      // A live subscription with no selected row is confirmed absence, not loading.
      if (snapshot.status === "live" && snapshot.data === undefined)
        data = undefined;
      appliedVersion = snapshot.appliedVersion;
      versionAtom.reportChanged();
      const next =
        snapshot.status === "live" && snapshot.data === undefined
          ? { status: "absent" as const }
          : snapshot.error !== null && data === undefined
            ? { status: "error" as const, error: snapshot.error }
            : data === undefined
              ? { status: "loading" as const }
              : {
                  status:
                    snapshot.status === "live"
                      ? ("live" as const)
                      : ("stale" as const),
                  data: data as DeepReadonly<typeof data>,
                  ...(snapshot.error !== null ? { error: snapshot.error } : {}),
                };
      if (
        current.status === next.status &&
        (!("data" in next) ||
          ("data" in current && Object.is(current.data, next.data))) &&
        (!("error" in current) ||
          ("error" in next && current.error === next.error)) &&
        (!("error" in next) ||
          ("error" in current && current.error === next.error))
      )
        return;
      current = next;
      atom.reportChanged();
    };
    const inactive = action(() => {
      entry.observed = false;
      if (!ref && releaseOnUnobserved) {
        if (request) releaseAfterRead = true;
        else
          schedule(() => {
            if (entries.get(key) === entry && !entry.observed && !request)
              entry.evict();
          });
      } else
        schedule(() => {
          if (entry.observed) return;
          if (ref && current.status === "live") {
            current = { status: "stale", data: current.data };
            atom.reportChanged();
          }
          trim();
        });
    });
    const interest = observeInterest(() => {
      if (!ref) return () => {};
      const stop = subscriptions.subscribe(ref, () => schedule(update));
      schedule(update);
      return stop;
    }, inactive);
    const atom = createAtom(key);
    onBecomeObserved(atom, () => {
      if (disposed) return;
      entry.observed = true;
      releaseAfterRead = false;
      touch(key);
      if (ref) interest.observed();
      else if (current.status !== "live") void read().catch(() => {});
    });
    onBecomeUnobserved(atom, ref ? interest.unobserved : inactive);
    // Metadata has separate observers so identical data publications stay cheap.
    const versionAtom = createAtom(`${key}:applied-version`);
    const entry: Entry = {
      resourceKey: ref ? subscriptionKey(ref) : key,
      observed: false,
      keyed: remove !== undefined,
      isReading: () => request !== undefined,
      dispose() {
        if (ref) interest.dispose();
      },
      evict: action(() => {
        generation++;
        request = undefined;
        pending.delete(update);
        data = undefined;
        appliedVersion = undefined;
        versionAtom.reportChanged();
        current = { status: "loading" };
        atom.reportChanged();
        if (remove && !entry.observed) {
          entries.delete(key);
          remove();
          if (ref) interest.dispose();
        }
      }),
    };
    function read(): Promise<TData> {
      if (!("read" in definition))
        throw new Error("This resource requires observation");
      releaseAfterRead = false;
      if (request) return request;
      const load = definition.read.bind(definition);
      const attempt = ++generation;
      request = flow(function* () {
        try {
          // Defer IO so an explicit read can join the observation's in-flight request.
          yield Promise.resolve();
          if (!disposed && generation === attempt) {
            current = { status: "loading" };
            atom.reportChanged();
          }
          const value: TData = yield load();
          if (!disposed && generation === attempt) {
            current = { status: "live", data: value };
            atom.reportChanged();
          }
          return value;
        } catch (error) {
          if (!disposed && generation === attempt) {
            current = {
              status: "error",
              error: error instanceof Error ? error : new Error(String(error)),
            };
            atom.reportChanged();
          }
          throw error;
        } finally {
          if (generation === attempt) {
            request = undefined;
            if (releaseAfterRead) entry.evict();
            else trim();
          }
        }
      })();
      return request;
    }
    entries.set(key, entry);
    return {
      read,
      get current() {
        atom.reportObserved();
        return current;
      },
      get appliedVersion() {
        atom.reportObserved();
        versionAtom.reportObserved();
        return disposed ? undefined : appliedVersion;
      },
    };
  }
  const runtime = {
    slot,
    keyed,
  } satisfies ObservableResourceRuntime;
  function keyed<TData>(definition: {
    read: true;
    releaseOnUnobserved?: boolean;
  }): (ref: ReadIntent<TData>) => ReadSlot<TData>;
  function keyed<R extends ResourceRef, TData>(
    definition: ResourceDefinition<R, TData>,
  ): (ref: R) => ResourceSlot<DeepReadonly<TData>>;
  function keyed<R extends ResourceRef, TData>(
    definition:
      | ResourceDefinition<R, TData>
      | { read: true; releaseOnUnobserved?: boolean },
  ) {
    type Cached = {
      slot: ReadSlot<TData | DeepReadonly<TData>>;
      facade: ReadSlot<TData | DeepReadonly<TData>>;
    };
    const cache = new Map<string, Cached>();
    const lookup = (ref: R | ReadIntent<TData>): Cached => {
      if (disposed) throw new Error("The observable resource store is closed.");
      const key = "key" in ref ? ref.key : subscriptionKey(ref);
      let entry = cache.get(key);
      if (!entry) {
        const inner =
          "read" in definition && "key" in ref
            ? slot(ref, () => cache.delete(key), definition.releaseOnUnobserved)
            : !("read" in definition) && !("key" in ref)
              ? slot({ ...definition, key, ref }, () => cache.delete(key))
              : (() => {
                  throw new Error("Invalid resource definition");
                })();
        const created: Cached = {
          slot: inner,
          facade: {
            read: () => lookup(ref).slot.read(),
            get current() {
              return (disposed ? created : lookup(ref)).slot.current;
            },
            get appliedVersion() {
              return (disposed ? created : lookup(ref)).slot.appliedVersion;
            },
          },
        };
        entry = created;
        cache.set(key, entry);
      }
      touch(key);
      trim();
      return entry;
    };
    return (ref: R | ReadIntent<TData>) => lookup(ref).facade;
  }

  return {
    ...runtime,
    dispose() {
      disposed = true;
      pending.clear();
      stopEvictions();
      for (const entry of entries.values()) {
        entry.dispose();
        entry.evict();
      }
      entries.clear();
    },
  };
}
