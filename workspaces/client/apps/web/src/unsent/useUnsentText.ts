import { useEffect, useMemo } from "react";
import { flowResult, makeAutoObservable } from "mobx";
import { useServerServices } from "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { serverViewId } from "../servers/environment.ts";
import { reportUnsentFailure, unsentStore, type TextFeature } from "./store.ts";

/** One editing subject's device buffer. Writes retain the existing close barrier. */
export class UnsentText {
  value: string | null = null;
  changed = false;
  #store: ReturnType<typeof unsentStore>;
  #id: string;
  constructor(server: string, feature: TextFeature, id: string) {
    this.#store = unsentStore(server, feature);
    this.#id = id;
    makeAutoObservable(this, {}, { autoBind: true });
  }
  start() {
    let active = true;
    void flowResult(this.restore(() => active)).catch(reportUnsentFailure);
    return () => {
      active = false;
    };
  }
  *restore(
    active: () => boolean,
  ): Generator<Promise<string | undefined>, void, string | undefined> {
    const saved = yield this.#store.get(this.#id);
    if (active() && !this.changed && typeof saved === "string")
      this.value = saved;
  }
  clear(expected = this.value) {
    this.changed = true;
    return expected === null
      ? Promise.resolve()
      : this.#store
          .deleteIfEqual(this.#id, expected)
          .catch((error: unknown) => {
            reportUnsentFailure();
            throw error;
          });
  }
  update(next: string | null) {
    const previous = this.value;
    this.changed = true;
    this.value = next;
    void (
      next === null ? this.clear(previous) : this.#store.put(this.#id, next)
    ).catch(reportUnsentFailure);
  }
}

export function useUnsentText(feature: TextFeature, id: string) {
  const server = useServerServices()?.owner.id ?? serverViewId ?? "local";
  const owner = useMemo(
    () => new UnsentText(server, feature, id),
    [server, feature, id],
  );
  useEffect(() => owner.start(), [owner]);
  return [owner.value, owner.update, owner.clear] as const;
}
