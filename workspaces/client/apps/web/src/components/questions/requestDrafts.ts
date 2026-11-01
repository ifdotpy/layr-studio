import { unsentStore } from "../../unsent/store";
import type { AnswerValues } from "./AnswerFields";

// Secret answers live only in memory, owned by the same immutable storage
// instance as the request's persisted public answers.
const drafts = new WeakMap<Storage, Map<string, AnswerValues>>();

export function requestDraftsForStorage(storage: Storage) {
  let values = drafts.get(storage);
  if (!values) {
    values = new Map<string, AnswerValues>();
    drafts.set(storage, values);
  }
  return values;
}

/** Remove a confirmed answer or deleted question from memory and the buffer. */
export async function removeAnswerDraft(
  storage: Storage,
  server: string,
  key: string,
  submitted?: string,
) {
  requestDraftsForStorage(storage).delete(key);
  const buffer = unsentStore(server, "answer");
  if (submitted === undefined) await buffer.delete(key);
  else await buffer.deleteIfEqual(key, submitted);
}
