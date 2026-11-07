import { useMemo } from "react";
import { computed, compareShallow } from "mobx";
import { transcriptRequests } from "./questionCardModel";

export function useTranscriptRequests(
  items: Parameters<typeof transcriptRequests>[0],
  requests: Parameters<typeof transcriptRequests>[1],
  cards: Parameters<typeof transcriptRequests>[2],
) {
  return useMemo(
    () =>
      computed(() => transcriptRequests(items, requests, cards), {
        equals: compareShallow,
      }),
    [items, requests, cards],
  ).get();
}
