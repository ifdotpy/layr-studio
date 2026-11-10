import { makeAutoObservable, runInAction } from "mobx";
import { errorText } from "../../apiErrors.ts";
import type { ConnectionHealth } from "../../sync/connectionState.ts";
export class NetStatus {
  connection: "live" | "reconnecting" | "offline" | "schema-mismatch" =
    "reconnecting";
  inFlight = 0;
  lastError: string | null = null;
  constructor() {
    makeAutoObservable(this);
  }
  receive(connection: ConnectionHealth) {
    if (this.connection === "schema-mismatch") return;
    this.connection =
      connection === "live" ||
      connection === "offline" ||
      connection === "schema-mismatch"
        ? connection
        : "reconnecting";
  }
  begin() {
    this.inFlight++;
    let finished = false;
    return (error?: unknown) =>
      runInAction(() => {
        if (finished) return;
        finished = true;
        if (error !== undefined) this.lastError = errorText(error);
        this.inFlight--;
      });
  }
}

export const defaultNetStatus = new NetStatus();
