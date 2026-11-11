import { useState } from "react";
import { flowResult, makeAutoObservable } from "mobx";
import type { get } from "../api.ts";

/** Forced attempts settle independently within one credential-bound API. */
export class LimitRefresh {
  pending = new Map<string, number>();
  cached = new Map<string, Promise<void>>();
  #read: typeof get;

  constructor(read: typeof get) {
    this.#read = read;
    makeAutoObservable(this, {}, { autoBind: true });
  }

  isRefreshing(key: string) {
    return !!this.pending.get(key);
  }

  reload(key: string, force = false): Promise<void> {
    const cached = !force && this.cached.get(key);
    if (cached) return cached;
    const request = flowResult(this.load(key, force));
    if (!force) this.cached.set(key, request);
    return request;
  }

  *load(key: string, force: boolean): Generator<ReturnType<typeof get>, void> {
    this.pending.set(key, (this.pending.get(key) ?? 0) + 1);
    try {
      yield this.#read("/api/limits", {
        query: {
          ...(key === "default" ? {} : { account_key: key }),
          ...(force ? {} : { cached: "1" as const }),
        },
        timeoutMs: 25000,
      });
    } catch {
      // Limit observations carry refresh failures through the server stream.
    } finally {
      const count = (this.pending.get(key) ?? 1) - 1;
      if (count) this.pending.set(key, count);
      else this.pending.delete(key);
      if (!force) this.cached.delete(key);
    }
  }
}

// integration: callers construct LimitRefresh in their existing owner.
export function useLimitRefresh(read: typeof get) {
  const [refresh] = useState(() => new LimitRefresh(read));
  return refresh;
}
