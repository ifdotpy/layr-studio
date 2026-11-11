import type { SidebarBackend } from "../../components/sidebar/support/services.tsx";
import { makeAutoObservable, observableRef } from "mobx";
import { outboxRequests } from "../../browserPersistence.ts";
import {
  ApiError,
  errorText,
  type ApiPostPath,
  type PostBody,
} from "../../api.ts";

/** One form's immutable request, including an uncertain or acknowledged retry. */
export class ProjectSave<Path extends ApiPostPath> {
  pending = false;
  frozen = false;
  error = "";
  request: PostBody<Path> | null = null;
  acknowledged = false;
  #backend: SidebarBackend;
  #path: Path;
  #onSaved: () => Promise<void>;
  #storageKey: string | undefined;
  #outbox: ReturnType<typeof outboxRequests>;
  constructor(
    backend: SidebarBackend,
    path: Path,
    onSaved: () => Promise<void>,
    storageKey?: string,
  ) {
    this.#backend = backend;
    this.#path = path;
    this.#onSaved = onSaved;
    this.#storageKey = storageKey;
    this.#outbox = outboxRequests(backend.storage);
    try {
      const stored = storageKey
        ? JSON.parse(this.#outbox.get("projectSave", storageKey) || "null")
        : null;
      this.request = stored?.body ?? null;
      this.acknowledged = !!stored?.acknowledged;
      this.frozen = !!stored;
    } catch {
      /* Unreadable local requests cannot be restored. */
    }
    makeAutoObservable(this, { request: observableRef }, { autoBind: true });
  }
  get retryLabel() {
    if (!this.frozen) return null;
    return this.acknowledged ? "Refresh project" : "Retry saved request";
  }
  persist() {
    if (this.#storageKey)
      this.#outbox.set(
        "projectSave",
        this.#storageKey,
        JSON.stringify({ body: this.request, acknowledged: this.acknowledged }),
      );
  }
  *submit(body: PostBody<Path>): Generator<Promise<unknown>, void, unknown> {
    if (this.pending) return;
    this.pending = this.frozen = true;
    this.error = "";
    this.request ??= body;
    try {
      this.persist();
      if (!this.acknowledged) {
        yield this.#backend.post(this.#path, this.request, {
          timeoutMs: 15000,
        });
        this.acknowledged = true;
        this.persist();
      }
      yield this.#onSaved();
      if (this.#storageKey)
        this.#outbox.remove("projectSave", this.#storageKey);
    } catch (failure) {
      if (
        !this.acknowledged &&
        failure instanceof ApiError &&
        failure.status >= 400 &&
        failure.status < 500 &&
        failure.status !== 408
      ) {
        this.request = null;
        if (this.#storageKey)
          this.#outbox.remove("projectSave", this.#storageKey);
        this.frozen = false;
      }
      this.error = errorText(failure);
    } finally {
      this.pending = false;
    }
  }
}
