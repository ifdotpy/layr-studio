import { makeAutoObservable, observableRef } from "mobx";
import { errorText } from "../../../api.ts";

type Add = (files: File[]) => Promise<void>;
type Notify = (message: string) => void;

/** Retains the failed upload's files and original destination for an explicit retry. */
export class AttachmentSave {
  error = "";
  pending: { files: File[]; add: Add } | null = null;

  constructor() {
    makeAutoObservable(this, { pending: observableRef }, { autoBind: true });
  }

  *save(
    files: File[] | (() => Promise<File[]>),
    add: Add,
    notify: Notify,
    reset: () => void,
  ): Generator<Promise<File[]> | Promise<void>, void, File[]> {
    if (typeof files === "function") {
      try {
        files = yield files();
      } catch (error) {
        notify(errorText(error));
        return;
      }
    }
    if (!files.length) return;
    const pending = { files, add };
    this.pending = pending;
    try {
      yield add(files);
      if (this.pending !== pending) return;
      this.pending = null;
      this.error = "";
      reset();
    } catch (error) {
      if (this.pending !== pending) return;
      this.error = errorText(error);
      notify(this.error);
    }
  }

  retry(notify: Notify, reset: () => void) {
    if (this.pending)
      void this.save(this.pending.files, this.pending.add, notify, reset);
  }
}
