import type { ServerServices } from "../../servers/serverServices.ts";
import { whileObserved } from "../../observed.ts";
import * as mobx from "mobx";
import { errorText, type ApiReadMetadata } from "../../api.ts";

type ReadWatcher = ReturnType<ServerServices["watchResourceReads"]>;
type WatchRead = (read: () => Promise<void>) => ReadWatcher;

export class WorkspaceResource<T> {
  data: T | null = null;
  error = "";
  loading = true;
  private stopWatch: ReadWatcher | undefined;
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
  readonly watch: WatchRead | undefined;
  constructor(
    read: WorkspaceResource<T>["read"],
    receive: (data: T) => void = () => {},
    watch?: WatchRead,
  ) {
    this.receive = receive;
    this.watch = watch;
    this.read = read;
    mobx.makeAutoObservable<this, "stopWatch">(
      this,
      {
        data: mobx.observableRef,
        read: false,
        receive: false,
        watch: false,
        stopWatch: false,
      },
      { autoBind: true },
    );
    whileObserved(this, "data", () => {
      this.start();
      if (!this.watch) void this.load();
      return this.dispose;
    });
  }
  dispose() {
    this.active = false;
    this.stopWatch?.();
    this.stopWatch = undefined;
    this.controller?.abort();
  }
  start() {
    this.active = true;
    this.stopWatch ??= this.watch?.(async () => {
      await mobx.flowResult(this.load(true));
    });
    return this.dispose;
  }
  *load(
    fromWatch = false,
  ): Generator<Promise<T | undefined>, void, T | undefined> {
    if (!this.active) return;
    if (!fromWatch && this.stopWatch) {
      this.stopWatch.refresh();
      return;
    }
    if (!fromWatch && this.busy) {
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
      if (this.active && !controller.signal.aborted) {
        this.error = errorText(error);
        if (fromWatch) throw error;
      }
    } finally {
      this.busy = false;
      if (this.active) {
        this.loading = false;
        if (!fromWatch && (this.again || controller.signal.aborted)) {
          this.again = false;
          void this.load();
        }
      }
    }
  }
}
