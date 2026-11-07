import { useServerUploads } from "../servers/ServerServicesContext";
import { useRef, useState, type SetStateAction } from "react";
import { errorText } from "../api";
import type { Attachment } from "./ComposerAttachments";

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

export function useAttachmentDrafts(
  key: string,
  notify: (message: string) => void,
) {
  const { attachmentCancelled: cancelled } = useServerUploads();
  const scopes = useRef(new Map<string, Attachments>());
  const [, render] = useState(0);
  const current = retained(scopes.current.get(key) ?? {}, cancelled);
  scopes.current.set(key, current);
  const setAttachments = async (
    change: SetStateAction<Attachments>,
  ): Promise<boolean> => {
    try {
      const previous = retained(scopes.current.get(key) ?? {}, cancelled);
      const next = typeof change === "function" ? change(previous) : change;
      scopes.current.set(key, retained(next, cancelled));
      render((version) => version + 1);
      return true;
    } catch (error) {
      notify(errorText(error));
      return false;
    }
  };
  return [current, setAttachments] as const;
}
