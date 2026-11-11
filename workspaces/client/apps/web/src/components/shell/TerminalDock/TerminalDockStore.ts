import { wait } from "./ShellTerminalStore.ts";
import type { ScreenChannel } from "../../../servers/screenChannel.ts";
import { requestId } from "studio-claude-bridge/request-id.ts";
import { makeAutoObservable } from "mobx";
import { errorText, type PostBody, type PostResult } from "../../../api.ts";
import type * as Api from "../../../servers/net/api.ts";

/** One dock's interaction drafts and synchronous command locks. */
export class TerminalDockStore {
  query = "";
  scrollTop = 0;
  measuredHeight: number | null = null;
  measure(height: number) {
    this.measuredHeight = height;
  }
  scroll(value = 0) {
    this.scrollTop = value;
  }
  renameTitle: string | null = null;
  endTarget: string | null = null;
  pending = false;
  opened = false;
  height = 0;
  selected = "";
  creating = false;
  stop: (() => void) | undefined;
  readonly actions;
  constructor(
    actions: Pick<typeof Api, "post" | "save" | "saved"> & {
      notify(message: string): void;
    },
  ) {
    this.actions = actions;
    this.opened = actions.saved("codex.terminal.open", false);
    this.height = actions.saved("codex.terminal.height", 0);
    this.selected = actions.saved("codex.terminal.selected", "");
    makeAutoObservable(
      this,
      { actions: false, stop: false },
      { autoBind: true },
    );
  }
  start(screen: ScreenChannel | null, create: () => void) {
    this.dispose();
    const command = (operation: string) => {
      if (operation === "toggle") this.toggle();
      else if (operation === "new") create();
    };
    if (screen) this.stop = screen.bindTerminal(command);
    else {
      const input = (event: Event) =>
        command((event as CustomEvent<string>).detail);
      window.addEventListener("studio-terminal-command", input);
      this.stop = () =>
        window.removeEventListener("studio-terminal-command", input);
    }
    return this.dispose;
  }
  dispose() {
    this.stop?.();
    this.stop = undefined;
  }
  open(value: boolean) {
    this.opened = value;
    if (!value) this.confirmEnd(null);
    this.actions.save("codex.terminal.open", value);
  }
  toggle = () => this.open(!this.opened);
  choose(value: string) {
    this.selected = value;
    this.select();
    this.actions.save("codex.terminal.selected", value);
  }
  resize(value: number) {
    this.height = value;
    this.actions.save("codex.terminal.height", value);
  }
  *create(agent: string, ready: () => void) {
    if (this.creating) return;
    this.creating = true;
    this.open(true);
    try {
      const shell = yield* wait(
        this.actions.post("/api/terminals/create", {
          id: requestId(),
          agent,
          cols: 100,
          rows: 24,
        }),
      );
      this.choose(`shell:${shell.id}`);
      this.search("");
      ready();
    } catch (error) {
      this.actions.notify(errorText(error));
    } finally {
      this.creating = false;
    }
  }
  search = (value: string) => (this.query = value);
  rename = (value: string | null) => (this.renameTitle = value);
  confirmEnd = (id: string | null) => (this.endTarget = id);
  select() {
    this.renameTitle = this.endTarget = null;
  }
  *action<Path extends "/api/terminals/rename" | "/api/terminals/close">(
    path: Path,
    body: PostBody<Path>,
  ): Generator<Promise<PostResult<Path>>, boolean, PostResult<Path>> {
    if (this.pending) return false;
    this.pending = true;
    try {
      yield this.actions.post(path, body);
      if (path === "/api/terminals/rename") this.renameTitle = null;
      return true;
    } catch (error) {
      this.actions.notify(errorText(error));
      return false;
    } finally {
      this.pending = false;
    }
  }
}
