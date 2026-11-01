import type { TranscriptPageData } from "../types";

/** Local transcript metadata format; item hashes remain server-owned. */
export const TRANSCRIPT_PROJECTION_FORMAT = 3;
/** Read persisted metadata written before the unused item revision map was removed. */
const LEGACY_TRANSCRIPT_PROJECTION_FORMAT = 2;

export type TranscriptProjectionMeta = {
  format: typeof TRANSCRIPT_PROJECTION_FORMAT;
  epoch?: string | undefined;
  metadata: Omit<Partial<TranscriptPageData>, "items">;
  order: string[];
};

export function readTranscriptMeta(
  payload: string,
): TranscriptProjectionMeta | null {
  try {
    const value = JSON.parse(payload);
    if (
      (value?.format === TRANSCRIPT_PROJECTION_FORMAT ||
        value?.format === LEGACY_TRANSCRIPT_PROJECTION_FORMAT) &&
      value.metadata &&
      Array.isArray(value.order)
    )
      return {
        format: TRANSCRIPT_PROJECTION_FORMAT,
        epoch: value.epoch,
        metadata: value.metadata,
        order: value.order,
      };
  } catch {
    return null;
  }
  return null;
}
