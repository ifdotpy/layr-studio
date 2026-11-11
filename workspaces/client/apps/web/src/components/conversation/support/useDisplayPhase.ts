import {
  action,
  compareShallow,
  makeAutoObservable,
  observableRef,
  reaction,
  type IReactionDisposer,
} from "mobx";

/** The selection owner starts this once and disposes it when its scope closes. */
export class DisplayPhase {
  shown: { scope: string | null; phase: string };
  private timer: ReturnType<typeof setTimeout> | undefined;
  private stop: IReactionDisposer | undefined;
  readonly read: () => readonly [string | null, string];
  constructor(read: DisplayPhase["read"]) {
    this.read = read;
    const [scope, phase] = read();
    this.shown = { scope, phase };
    makeAutoObservable<this, "timer" | "stop">(
      this,
      { read: false, shown: observableRef, timer: false, stop: false },
      { autoBind: true },
    );
  }
  get current() {
    const [scope, phase] = this.read();
    return this.shown.scope === scope ? this.shown.phase : phase;
  }
  start() {
    this.stop = reaction(
      this.read,
      ([scope, phase]) => {
        clearTimeout(this.timer);
        if (this.shown.scope !== scope) this.shown = { scope, phase };
        else if (this.shown.phase !== phase)
          this.timer = setTimeout(
            action(() => {
              this.shown = { scope, phase };
            }),
            250,
          );
      },
      { fireImmediately: true, equals: compareShallow },
    );
    return this.dispose;
  }
  dispose() {
    clearTimeout(this.timer);
    this.stop?.();
  }
}
