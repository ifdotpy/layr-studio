import { makeAutoObservable } from "mobx";
import { errorText, type PostBody, type PostResult } from "../../../api.ts";
import type * as Api from "../../../servers/net/api.ts";

/** One dock's interaction drafts and synchronous command locks. */
export class TerminalDockStore {
  query = "";
  renameTitle: string | null = null;
  endTarget: string | null = null;
  pending = false;
  readonly actions;
  constructor(
    actions: Pick<typeof Api, "post"> & { notify(message: string): void },
  ) {
    this.actions = actions;
    makeAutoObservable(this, { actions: false }, { autoBind: true });
  }
  search = (value: string) => (this.query = value);
  rename = (value: string | null) => (this.renameTitle = value);
  confirmEnd = (id: string | null) => (this.endTarget = id);
  select() {
    this.renameTitle = null;
    this.endTarget = null;
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
