import { makeAutoObservable, observableRef } from "mobx";
import type { SubscriptionCacheStorage } from "./subscriptionCache.ts";

export type CacheScope = Readonly<{
  serverId: string;
  credentialId: string;
  tokenFingerprint: string;
  workspaceId: string;
}>;
export const cacheScopePrefix = (scope: CacheScope) =>
  `scope:${encodeURIComponent(scope.credentialId)}:${scope.tokenFingerprint}:${scope.workspaceId}:`;

/** The sync client's one cache lifetime. Every asynchronous commit uses this epoch. */
export class CacheScopeOwner {
  current: CacheScope | null;
  epoch = 0;
  closed = false;
  private queue: Promise<unknown> = Promise.resolve();
  private readonly storage: SubscriptionCacheStorage | undefined;
  private readonly changed: () => void;
  private readonly credentialId: string;
  constructor(
    initial: CacheScope,
    storage: SubscriptionCacheStorage | undefined,
    changed: () => void,
  ) {
    this.current = Object.freeze({ ...initial });
    this.credentialId = initial.credentialId;
    this.storage = storage;
    this.changed = changed;
    makeAutoObservable<CacheScopeOwner, "storage" | "changed" | "queue">(
      this,
      { current: observableRef, storage: false, changed: false, queue: false },
      { autoBind: true },
    );
  }
  capture(requireScope = true) {
    const epoch = this.epoch;
    return () =>
      !this.closed &&
      epoch === this.epoch &&
      (!requireScope || this.current !== null);
  }
  close() {
    if (this.closed) return;
    this.closed = true;
    this.epoch++;
  }
  private run<T>(
    valid: () => boolean,
    task: () => Promise<T>,
  ): Promise<T | undefined> {
    const result = this.queue.then(() => (valid() ? task() : undefined));
    this.queue = result.catch(() => undefined);
    return result;
  }
  replace(next: CacheScope | null) {
    if (this.closed) return Promise.resolve();
    if (JSON.stringify(next) === JSON.stringify(this.current))
      return this.queue.then(() => {});
    this.epoch++;
    this.current = next ? Object.freeze({ ...next }) : null;
    const valid = this.capture(false);
    const cleared = this.run(valid, async () => {
      if (this.storage?.clearScope)
        await this.storage.clearScope(this.credentialId, valid);
      else if (this.storage?.clear && valid()) await this.storage.clear();
    }).then(
      () => {},
      () => {},
    );
    this.changed();
    return cleared;
  }
  scopedStorage(): SubscriptionCacheStorage | undefined {
    const scope = this.current,
      storage = this.storage;
    if (
      this.closed ||
      !scope?.tokenFingerprint ||
      !scope.workspaceId ||
      !storage
    )
      return undefined;
    const prefix = cacheScopePrefix(scope),
      valid = this.capture();
    return {
      get: async (key) => {
        if (!valid()) return undefined;
        await this.queue;
        if (!valid()) return undefined;
        const value = await storage.get(prefix + key);
        return valid() ? value : undefined;
      },
      put: async (key, value) => {
        await this.run(valid, () => storage.put(prefix + key, value, valid));
      },
      delete: async (key) => {
        await this.run(valid, () => storage.delete(prefix + key, valid));
      },
    };
  }
}
