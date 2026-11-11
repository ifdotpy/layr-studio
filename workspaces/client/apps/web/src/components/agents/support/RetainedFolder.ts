import { makeAutoObservable } from "mobx";
import { errorText } from "../../../api.ts";

/** Folder reveal feedback stays with its mounted retained-worktree row. */
export class RetainedFolder {
  error = "";
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true });
  }
  *open(path: string, reveal: (path: string) => Promise<unknown>) {
    this.error = "";
    try {
      yield reveal(path);
    } catch (failure) {
      this.error = errorText(failure);
    }
  }
}
