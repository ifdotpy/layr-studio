import type { GetOptions, GetResult } from "../api.ts";
import type { StudioServer } from "./registry.ts";

type Options = Pick<GetOptions<"/api/models">, "query" | "signal">;
type Models = GetResult<"/api/models">;
export type ServerModelReader = {
  readonly scope: string;
  read(options: Options): Promise<Models>;
};

/** One authenticated owner coalesces reads; cancellation belongs to each waiter. */
export function createModelReader(
  owner: Pick<StudioServer, "id" | "origin" | "credentialId">,
  read: (options: Options) => Promise<Models>,
): ServerModelReader {
  const pending = new Map<string, Promise<Models>>();
  return {
    scope: JSON.stringify([owner.id, owner.origin, owner.credentialId]),
    async read({ query, signal }: Options): Promise<Models> {
      signal?.throwIfAborted();
      const key = JSON.stringify(query ?? {});
      let work = pending.get(key);
      if (!work) {
        work = read({ query }).finally(() => pending.delete(key));
        pending.set(key, work);
      }
      if (!signal) return work;
      return new Promise((resolve, reject) => {
        const abort = () => reject(signal.reason);
        signal.addEventListener("abort", abort, { once: true });
        work
          .then(resolve, reject)
          .finally(() => signal.removeEventListener("abort", abort));
      });
    },
  };
}
