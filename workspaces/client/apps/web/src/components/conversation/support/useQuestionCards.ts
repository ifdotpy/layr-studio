import { resourceData } from "../../../store/runtime.ts";
import { computed, makeAutoObservable, type IComputedValue } from "mobx";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { Message } from "../../../types.ts";
import {
  isPendingQuestionRequest,
  questionCard,
  questionHistoryIndex,
} from "./questionCardModel.ts";
import type { createRequestsView } from "./requestView.ts";

/** Only the transcript row observes its computed card; caches belong to its server and family. */
export function createQuestionCards(
  store: ReturnType<typeof useServerStore>,
  requests: ReturnType<typeof createRequestsView>,
  agentId: string | undefined,
) {
  if (!agentId) return (item: Message) => questionCard(item);
  const view = makeAutoObservable({
    get lookup() {
      const current = resourceData(store.questionHistory().current);
      const entries = current
        ? [...current.questionHistory.values()].filter((entry) => {
            const owner = requests.agents?.get(entry.agent);
            return (owner?.rootId || entry.agent) === requests.rootId;
          })
        : [];
      const pending = requests.familyRequests.filter(isPendingQuestionRequest);
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
}
