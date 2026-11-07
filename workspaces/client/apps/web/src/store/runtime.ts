import { createAtom, runInAction } from "mobx";
import type { DeepReadonly } from "../deepReadonly.ts";
import type {
  ResourceData,
  ResourceRef,
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
      (entry) => entry.keyed && !entry.observed,
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
    queueMicrotask(() => {
      const batch = [...pending];
      pending.clear();
      if (!disposed) runInAction(() => batch.forEach((apply) => apply()));
    });
  };

  function slot<R extends ResourceRef, TData>(
    {
      key,
      ref,
      create,
      apply,
    }: ResourceDefinition<R, TData> & { key: string; ref: R },
    remove?: () => void,
  ): ResourceSlot<DeepReadonly<TData>> {
    if (disposed) throw new Error("The observable resource store is closed.");
    if (entries.has(key)) throw new Error(`Duplicate resource slot: ${key}`);
    let data: ReturnType<typeof create> | undefined;
    let current: ResourceStatus<DeepReadonly<ReturnType<typeof create>>> = {
      status: "loading",
    };
    const update = () => {
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
    const interest = observeInterest(
      () => {
        const stop = subscriptions.subscribe(ref, () => schedule(update));
        schedule(update);
        return stop;
      },
      () => {
        entry.observed = false;
        runInAction(() => {
          if (current.status === "live") {
            current = { status: "stale", data: current.data };
            atom.reportChanged();
          }
        });
        trim();
      },
    );
    const atom = createAtom(
      key,
      () => {
        entry.observed = true;
        touch(key);
        interest.observed();
      },
      interest.unobserved,
    );
    const entry: Entry = {
      resourceKey: subscriptionKey(ref),
      observed: false,
      keyed: remove !== undefined,
      dispose: interest.dispose,
      evict() {
        pending.delete(update);
        runInAction(() => {
          data = undefined;
          current = { status: "loading" };
          atom.reportChanged();
        });
        if (remove && !entry.observed) {
          entries.delete(key);
          remove();
          interest.dispose();
        }
      },
    };
    entries.set(key, entry);
    return {
      get current() {
        atom.reportObserved();
        return current;
      },
    };
  }
  const runtime: ObservableResourceRuntime = {
    slot,
    keyed<R extends ResourceRef, TData>(
      definition: ResourceDefinition<R, TData>,
    ): (ref: R) => ResourceSlot<DeepReadonly<TData>> {
      type Cached = {
        slot: ResourceSlot<DeepReadonly<TData>>;
        facade: ResourceSlot<DeepReadonly<TData>>;
      };
      const cache = new Map<string, Cached>();
      const lookup = (ref: R): Cached => {
        if (disposed)
          throw new Error("The observable resource store is closed.");
        const key = subscriptionKey(ref);
        let entry = cache.get(key);
        if (!entry) {
          const inner = slot({ ...definition, key, ref }, () =>
            cache.delete(key),
          );
          const created: Cached = {
            slot: inner,
            facade: {
              get current() {
                return (disposed ? created : lookup(ref)).slot.current;
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
      return (ref) => lookup(ref).facade;
    },
  };
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
