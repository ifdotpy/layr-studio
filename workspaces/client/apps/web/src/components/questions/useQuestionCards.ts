import { useMemo } from "react";
import { computed, makeAutoObservable, type IComputedValue } from "mobx";
import { useServerStore } from "../../servers/ServerServicesContext";
import type { Message } from "../../types";
import {
  isPendingQuestionRequest,
  questionCard,
  questionHistoryIndex,
} from "./questionCardModel";
import { useRequestsView } from "./requestView";

/** A stable selector; only the transcript row observes its computed card. */
export function useQuestionCards(agentId: string | undefined) {
  const store = useServerStore();
  const requests = useRequestsView({ agentId, includeTeam: true });
  return useMemo(() => {
    if (!agentId) return (item: Message) => questionCard(item);
    const view = makeAutoObservable({
      get lookup() {
        const current = store.questionHistory().current;
        const entries =
          current.status === "live" || current.status === "stale"
            ? [...current.data.questionHistory.values()].filter((entry) => {
                const owner = requests.agents?.get(entry.agent);
                return (owner?.rootId || entry.agent) === requests.rootId;
              })
            : [];
        const pending = requests.familyRequests.filter(
          isPendingQuestionRequest,
        );
        return {
          agentId,
          history: questionHistoryIndex(entries),
          pending: new Map(
            [...questionHistoryIndex(pending)].map(([key, request]) => [
              key,
              request.id,
            ]),
          ),
          pendingIds: new Set(pending.map((request) => request.id)),
        };
      },
    });
    const cards = new WeakMap<
      Message,
      IComputedValue<ReturnType<typeof questionCard>>
    >();
    return (item: Message) => {
      let card = cards.get(item);
      if (!card) {
        card = computed(() => questionCard(item, view.lookup));
        cards.set(item, card);
      }
      return card.get();
    };
  }, [store, requests, agentId]);
}
