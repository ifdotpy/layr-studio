import { useState } from "react";
import { makeAutoObservable, runInAction } from "mobx";
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
    this.pending.set(key, (this.pending.get(key) ?? 0) + 1);
    const request = this.#read("/api/limits", {
      query: {
        ...(key === "default" ? {} : { account_key: key }),
        ...(force ? {} : { cached: "1" as const }),
      },
      timeoutMs: 25000,
    })
      .then(
        () => undefined,
        () => undefined,
      )
      .finally(() =>
        runInAction(() => {
          const count = (this.pending.get(key) ?? 1) - 1;
          if (count) this.pending.set(key, count);
          else this.pending.delete(key);
          if (this.cached.get(key) === request) this.cached.delete(key);
        }),
      );
    if (!force) this.cached.set(key, request);
    return request;
  }
}
export function useLimitRefresh(read: typeof get) {
  const [refresh] = useState(() => new LimitRefresh(read));
  return refresh;
}
