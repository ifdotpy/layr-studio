import { whileObserved } from "../../../observed.ts";
import { makeAutoObservable, observableRef } from "mobx";
import { readNewChatResource } from "../../../app/projects/newChatSettings.ts";
import type { GetResult } from "../../../api.ts";
import type { StudioServer } from "../../../servers/registry.ts";
type Summary = GetResult<"/api/ui-summary">;

/** One immutable connection's capability read, active only while displayed. */
export class ProjectServerRead {
  value: Summary | undefined;
  error = "";
  private controller: AbortController | undefined;
  constructor(connection?: StudioServer) {
    makeAutoObservable<this, "controller">(
      this,
      {
        value: observableRef,
        controller: false,
      },
      { autoBind: true },
    );
    whileObserved(this, "value", () => {
      if (!this.value) void this.load(connection);
      return () => this.controller?.abort();
    });
  }
  *load(connection?: StudioServer): Generator<Promise<Summary>, void, Summary> {
    const controller = (this.controller = new AbortController());
    this.error = "";
    try {
      const value = yield readNewChatResource(connection, "/api/ui-summary", {
        signal: controller.signal,
      });
      if (!controller.signal.aborted) this.value = value;
    } catch (error) {
      if (!controller.signal.aborted) this.error = String(error);
    }
  }
}
