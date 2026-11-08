import { makeAutoObservable, observableRef } from "mobx";
import type { ScreenChannel } from "../../servers/screenChannel.ts";
import type { ScreenSource } from "../contracts.ts";

/** The shell changes selection at its event source; server owners read it directly. */
export class ScreenSelection {
  selected: string;
  intent = 0;
  available: readonly string[];
  constructor(available: readonly string[]) {
    this.available = available;
    this.selected = available[0] || "";
    makeAutoObservable(this, { available: observableRef }, { autoBind: true });
  }
  get current() {
    return this.available.includes(this.selected)
      ? this.selected
      : this.available[0] || "";
  }
  updateServers(ids: readonly string[]) {
    this.available = ids;
  }
  select(id: string) {
    this.selected = id;
    this.intent++;
  }
}

export class ScreenOwner implements ScreenSource {
  channel: ScreenChannel | null = null;
  isActive: () => boolean = () => true;
  constructor() {
    makeAutoObservable(
      this,
      { channel: observableRef, isActive: observableRef },
      { autoBind: true },
    );
  }
  get active() {
    return this.isActive();
  }
  connect(channel: ScreenChannel | null, isActive: () => boolean) {
    this.channel = channel;
    this.isActive = isActive;
  }
}
