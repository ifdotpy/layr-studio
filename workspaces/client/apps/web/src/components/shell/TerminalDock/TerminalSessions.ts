import { makeAutoObservable } from "mobx";
import { errorText } from "../../../api.ts";
import { TerminalState } from "../../../generated/api.ts";
import { resourceData } from "../../../store/runtime.ts";
import type { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { running } from "./ShellTerminalStore.ts";
import type { TerminalDockStore } from "./TerminalDockStore.ts";

/** One mounted dock derives its filtered, virtualized sessions from its server. */
export class TerminalSessions {
  readonly server;
  readonly ui;
  // Session row height in CSS pixels, shared by virtualization and rendering.
  readonly rowHeight = 58;
  constructor(
    server: ReturnType<typeof useServerStore>,
    ui: TerminalDockStore,
  ) {
    this.server = server;
    this.ui = ui;
    makeAutoObservable(this, { server: false, ui: false }, { autoBind: true });
  }
  get list() {
    return this.server.terminals().current;
  }
  get error() {
    return "error" in this.list ? errorText(this.list.error) : "";
  }
  get entries() {
    return (resourceData(this.list) ?? []).filter(
      (shell) => shell.status !== TerminalState.closed,
    );
  }
  get current() {
    return this.entries.find(
      (entry) => `shell:${entry.id}` === this.ui.selected,
    );
  }
  owner(id: string) {
    return (
      resourceData(this.server.state().current)?.agents.get(id)?.name || "Agent"
    );
  }
  get filtered() {
    return this.entries.filter((entry) =>
      `${entry.title} ${entry.cwd || ""} ${this.owner(entry.agent)} ${entry.status}`
        .toLowerCase()
        .includes(this.ui.query.toLowerCase()),
    );
  }
  get start() {
    return Math.max(
      0,
      Math.min(
        Math.max(0, this.filtered.length - 1),
        Math.floor(this.ui.scrollTop / this.rowHeight) - 3,
      ),
    );
  }
  get visible() {
    return this.filtered.slice(
      this.start,
      this.start +
        Math.ceil((this.ui.measuredHeight ?? this.ui.height) / this.rowHeight) +
        6,
    );
  }
  get activeCount() {
    return this.entries.filter((entry) => running(entry.status)).length;
  }
}
