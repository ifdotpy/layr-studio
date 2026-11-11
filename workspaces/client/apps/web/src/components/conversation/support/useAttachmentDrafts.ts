import { whileObserved } from "../../../observed.ts";
import type { SetStateAction } from "react";
import { makeAutoObservable, observableRef } from "mobx";
import { errorText } from "../../../api.ts";
import type { Attachment } from "../ComposerAttachments/ComposerAttachments.tsx";

type Attachments = Record<string, Attachment[]>;
function retained(
  stored: Attachments,
  attachmentCancelled: (id: string) => boolean,
): Attachments {
  let result = stored;
  for (const [id, files] of Object.entries(stored)) {
    if (!files.some((asset) => attachmentCancelled(asset.id))) continue;
    if (result === stored) result = { ...stored };
    result[id] = files.filter((asset) => !attachmentCancelled(asset.id));
  }
  return result;
}

export class AttachmentDrafts {
  current: Attachments = {};
  cancelled: (id: string) => boolean;
  notify: (message: string) => void;
  options:
    | {
        key: string;
        storage: Storage;
        eventKey?: (event: StorageEvent) => string | null | undefined;
      }
    | undefined;
  constructor(
    cancelled: (id: string) => boolean,
    notify: (message: string) => void,
    options?: AttachmentDrafts["options"],
  ) {
    this.options = options;
    this.cancelled = cancelled;
    this.notify = notify;
    makeAutoObservable(
      this,
      {
        current: observableRef,
        cancelled: false,
        notify: false,
        options: false,
      },
      { autoBind: true },
    );
    this.read();
    const changed = (event: StorageEvent) => {
      if (
        (options?.eventKey ? options.eventKey(event) : event.key) ===
          options?.key ||
        event.key === null
      )
        this.read();
    };
    whileObserved(this, "current", () => {
      this.read();
      window.addEventListener("storage", changed);
      return () => window.removeEventListener("storage", changed);
    });
  }
  read() {
    try {
      if (this.options)
        this.current = retained(
          JSON.parse(this.options.storage.getItem(this.options.key) || "{}"),
          this.cancelled,
        );
    } catch (error) {
      this.notify(errorText(error));
    }
  }
  set(change: SetStateAction<Attachments>): Promise<boolean> {
    try {
      this.read();
      const previous = retained(this.current, this.cancelled);
      this.current = retained(
        typeof change === "function" ? change(previous) : change,
        this.cancelled,
      );
      if (this.options)
        this.options.storage.setItem(
          this.options.key,
          JSON.stringify(this.current),
        );
      return Promise.resolve(true);
    } catch (error) {
      this.notify(errorText(error));
      return Promise.resolve(false);
    }
  }
}
