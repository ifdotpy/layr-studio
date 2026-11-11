import { whileObserved } from "../observed.ts";
import { flowValue } from "../flowValue.ts";
import {
  action,
  computed,
  flow,
  flowResult,
  makeAutoObservable,
  observable,
  isFlowCancellationError,
  type IComputedValue,
} from "mobx";
import { useServerAreas } from "../app/ServerAreasProvider.tsx";
import { reportUnsentFailure, unsentStore, type TextFeature } from "./store.ts";

/** Each mounted editor observes its own durable text through the server owner. */
export class UnsentText {
  readonly server;
  readonly values = observable.map<string, string | null>();
  readonly #changed = new Set<string>();
  readonly #views = new Map<string, IComputedValue<string | null>>();
  constructor(server: string) {
    this.server = server;
    makeAutoObservable(this, {}, { autoBind: true });
  }
  key(feature: TextFeature, id: string) {
    return JSON.stringify([feature, id]);
  }
  *restore(
    feature: TextFeature,
    id: string,
  ): Generator<Promise<string | undefined>, void, string | undefined> {
    const key = this.key(feature, id);
    const saved = yield unsentStore(this.server, feature).get(id);
    if (this.#changed.has(key)) return;
    if (saved === undefined) this.values.delete(key);
    else this.values.set(key, saved);
  }
  cached(feature: TextFeature, id: string) {
    return this.values.get(this.key(feature, id)) ?? null;
  }
  read(feature: TextFeature, id: string) {
    const key = this.key(feature, id);
    let view = this.#views.get(key);
    if (!view) {
      view = computed(() => this.cached(feature, id));
      this.#changed.delete(key);
      const release = action(() => {
        this.#views.delete(key);
        this.#changed.delete(key);
        this.values.delete(key);
      });
      whileObserved(
        view,
        undefined,
        () => {
          const request = flowResult(this.restore(feature, id));
          void request.catch((error: unknown) => {
            if (!(error instanceof Error && isFlowCancellationError(error)))
              reportUnsentFailure();
          });
          return action(() => {
            request.cancel();
            release();
          });
        },
        release,
      );
      this.#views.set(key, view);
    }
    return view.get();
  }
  clear = flow(function* (
    this: UnsentText,
    feature: TextFeature,
    id: string,
    expected = this.cached(feature, id),
  ) {
    this.#changed.add(this.key(feature, id));
    if (expected === null) return;
    try {
      yield* flowValue(
        unsentStore(this.server, feature).deleteIfEqual(id, expected),
      );
      if (this.cached(feature, id) === expected)
        this.values.set(this.key(feature, id), null);
    } catch (error) {
      reportUnsentFailure();
      throw error;
    }
  });
  update(feature: TextFeature, id: string, next: string | null) {
    const previous = this.cached(feature, id);
    this.#changed.add(this.key(feature, id));
    this.values.set(this.key(feature, id), next);
    void (
      next === null
        ? this.clear(feature, id, previous)
        : unsentStore(this.server, feature).put(id, next)
    ).catch(reportUnsentFailure);
  }
}

export function useUnsentText(feature: TextFeature, id: string) {
  const owner = useServerAreas().unsentText;
  return [
    owner.read(feature, id),
    (value: string | null) => owner.update(feature, id, value),
    (expected?: string | null) => owner.clear(feature, id, expected),
  ] as const;
}
