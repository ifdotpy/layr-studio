import { whileObserved } from "../../../observed.ts";
import * as mobx from "mobx";
import { errorText, type ApiReadMetadata } from "../../../api.ts";

// Changes, checkpoints, profiles, rules and capabilities lack an SSE resource
// owner. Keep explicit reads and command rereads until that server contract exists.
export class WorkspaceResource<T> {
  data: T | null = null;
  error = "";
  loading = true;
  private active = false;
  private busy = false;
  private again = false;
  private controller: AbortController | undefined;
  private etag: string | undefined;
  readonly read: (
    signal: AbortSignal,
    etag: string | undefined,
    metadata: ApiReadMetadata,
  ) => Promise<T | undefined>;
  readonly receive: (data: T) => void;
  constructor(
    read: WorkspaceResource<T>["read"],
    receive: (data: T) => void = () => {},
  ) {
    this.receive = receive;
    this.read = read;
    mobx.makeAutoObservable(
      this,
      { data: mobx.observableRef, read: false, receive: false },
      { autoBind: true },
    );
    whileObserved(this, "data", () => {
      this.start();
      void this.load();
      return this.dispose;
    });
  }
  dispose() {
    this.active = false;
    this.controller?.abort();
  }
  start() {
    this.active = true;
    return this.dispose;
  }
  *load(): Generator<Promise<T | undefined>, void, T | undefined> {
    if (!this.active) return;
    if (this.busy) {
      this.again = true;
      return;
    }
    this.busy = true;
    const controller = new AbortController();
    this.controller = controller;
    this.loading = this.data === null;
    const metadata: ApiReadMetadata = {};
    try {
      const data = yield this.read(controller.signal, this.etag, metadata);
      if (!this.active || controller.signal.aborted) return;
      this.etag = metadata.etag;
      if (!metadata.notModified && data !== undefined) {
        this.data = data;
        this.receive(data);
      }
      this.error = "";
    } catch (error) {
      if (this.active && !controller.signal.aborted)
        this.error = errorText(error);
    } finally {
      this.busy = false;
      if (this.active) {
        this.loading = false;
        if (this.again || controller.signal.aborted) {
          this.again = false;
          void this.load();
        }
      }
    }
  }
}
