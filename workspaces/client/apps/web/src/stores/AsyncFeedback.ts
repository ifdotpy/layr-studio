import { makeAutoObservable } from "mobx";
import { errorText } from "../apiErrors";

/**
 * Pending and error feedback for one user-triggered async action. It adds no
 * request identity or retry: callers keep those in their existing owners.
 */
export class AsyncFeedback {
  pending = false;
  error = "";

  constructor() {
    makeAutoObservable(this, {}, { autoBind: true });
  }

  /** Runs `work` unless one run is pending; `accept` receives its result. */
  *run<T>(
    work: () => Promise<T>,
    accept?: (value: T) => void,
  ): Generator<Promise<T>, void, T> {
    if (this.pending) return;
    this.pending = true;
    this.error = "";
    try {
      const value = yield work();
      accept?.(value);
    } catch (error) {
      this.error = errorText(error);
    } finally {
      this.pending = false;
    }
  }
}
