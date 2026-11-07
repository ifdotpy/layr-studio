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
  | { readonly status: "error"; readonly error: SubscriptionError }
  | { readonly status: "live"; readonly data: T }
  | {
      readonly status: "stale";
      readonly data: T;
      readonly error?: SubscriptionError;
    };

export interface ResourceSlot<T> {
  readonly current: ResourceStatus<T>;
}

export interface ObservableResourceRuntime {
  slot<R extends ResourceRef, TData>(definition: {
    key: string;
    ref: R;
    create(dto: DeepReadonly<ResourceData<R>>): TData;
    apply(data: TData, dto: DeepReadonly<ResourceData<R>>): void;
  }): ResourceSlot<DeepReadonly<TData>>;
}

type Subscriptions = Pick<
  ReturnType<typeof createSubscriptions>,
  "subscribe" | "getSnapshot" | "onEvict"
>;

/** One runtime belongs to one server. Generated facades retain typed slot
 * identities; this map retains only their lifecycle, never erased model values. */
export function createObservableResourceRuntime(subscriptions: Subscriptions) {
  const entries = new Map<string, { dispose(): void; evict(): void }>();
  const pending = new Set<() => void>();
  let disposed = false;
  const stopEvictions = subscriptions.onEvict((ref) => {
    entries.get(subscriptionKey(ref))?.evict();
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

  const runtime: ObservableResourceRuntime = {
    slot({ key, ref, create, apply }) {
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
          else apply(data, dto);
        }
        const next =
          snapshot.error !== null && data === undefined
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
          if (current.status !== "live") return;
          runInAction(() => {
            if (current.status === "live") {
              current = { status: "stale", data: current.data };
              atom.reportChanged();
            }
          });
        },
      );
      const atom = createAtom(key, interest.observed, interest.unobserved);
      entries.set(key, {
        dispose: interest.dispose,
        evict() {
          pending.delete(update);
          runInAction(() => {
            data = undefined;
            current = { status: "loading" };
            atom.reportChanged();
          });
        },
      });
      return {
        get current() {
          atom.reportObserved();
          return current;
        },
      };
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
