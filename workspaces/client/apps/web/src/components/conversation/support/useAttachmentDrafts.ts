import { useServerUploads } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useRef, useLayoutEffect, type SetStateAction } from "react";
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
  constructor(
    cancelled: (id: string) => boolean,
    notify: (message: string) => void,
  ) {
    this.cancelled = cancelled;
    this.notify = notify;
    makeAutoObservable(
      this,
      { current: observableRef, cancelled: false, notify: false },
      { autoBind: true },
    );
  }
  discardCancelled() {
    this.current = retained(this.current, this.cancelled);
  }
  set(change: SetStateAction<Attachments>): Promise<boolean> {
    try {
      const previous = retained(this.current, this.cancelled);
      this.current = retained(
        typeof change === "function" ? change(previous) : change,
        this.cancelled,
      );
      return Promise.resolve(true);
    } catch (error) {
      this.notify(errorText(error));
      return Promise.resolve(false);
    }
  }
}
export function useAttachmentDraftStore(
  key: string,
  notify: (message: string) => void,
) {
  const { attachmentCancelled } = useServerUploads();
  const scopes = useRef(new Map<string, AttachmentDrafts>()).current;
  let store = scopes.get(key);
  if (!store) {
    store = new AttachmentDrafts(attachmentCancelled, notify);
    scopes.set(key, store);
  }
  useLayoutEffect(store.discardCancelled);
  return store;
}
export function useAttachmentDrafts(
  key: string,
  notify: (message: string) => void,
) {
  const store = useAttachmentDraftStore(key, notify);
  return [store.current, store.set] as const;
}
