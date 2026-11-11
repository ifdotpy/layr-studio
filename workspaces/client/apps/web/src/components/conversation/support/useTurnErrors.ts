import { resourceData } from "../../../store/runtime.ts";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { agentId as resourceAgentId } from "../../../generated/observableResources.ts";
import { makeAutoObservable } from "mobx";
import { derived } from "./derived.ts";
import { createTurnErrorMirror } from "../../../generated/observableResources.ts";
import {
  AnalyticsScope,
  AgentStatus as GeneratedApiAgentStatus,
} from "../../../generated/api.ts";
import type { DeepReadonly } from "../../../deepReadonly.ts";
import { useServerNet } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import "../../../api.ts";
import type { Agent, JsonInput, Message } from "../../../types.ts";
import { failureMessage } from "./turnFailureReason.ts";

export function failedTurnsNeedingLookup(
  items: Pick<Message, "turnId" | "turnStatus">[],
  existing: Set<string>,
): string[] {
  return [
    ...new Set(
      items.flatMap((item) =>
        ["failed", "uncertain"].includes(item.turnStatus || "") &&
        item.turnId &&
        !existing.has(item.turnId)
          ? [item.turnId]
          : [],
      ),
    ),
  ];
}

// Old servers saved terminal errors in analytics without transcript notices.
// The compact view also works with their full analytics response.
export class TurnErrorStore {
  readonly requested: string[];
  constructor(items: Message[]) {
    this.requested = openingTurns(items);
    makeAutoObservable(this, { requested: false, read: false });
  }
  read = derived(
    (
      items: Message[],
      agent: Agent | undefined,
      store: ReturnType<typeof useServerStore> | undefined,
      net: ReturnType<typeof useServerNet> | undefined,
    ) => readTurnErrors(items, agent, store, net, this.requested),
  );
}

function openingTurns(items: Message[]) {
  return failedTurnsNeedingLookup(
    items,
    new Set(
      items.flatMap((item) =>
        item.turnId &&
        (item.turnError ||
          item.turnErrorResolved ||
          item.nativeNotice === "error")
          ? [item.turnId]
          : [],
      ),
    ),
  ).slice(-120);
}

function readTurnErrors(
  items: Message[],
  agent: Agent | undefined,
  store: ReturnType<typeof useServerStore> | undefined,
  net: ReturnType<typeof useServerNet> | undefined,
  requested: string[],
) {
  const detail =
    agent && store
      ? resourceData(store.agentDetail(resourceAgentId(agent.id)).current)
      : undefined;
  const errors: Record<string, DeepReadonly<JsonInput>> = {};
  for (const item of items) {
    if (item.turnError && item.turnId) errors[item.turnId] = item.turnError;
  }
  const latest = detail?.nativeTurnError;
  if (latest?.turnId && latest.error) errors[latest.turnId] = latest.error;
  else if (
    agent?.status === GeneratedApiAgentStatus.failed &&
    !agent.turnId &&
    agent.lastCompletedTurn &&
    detail?.error
  )
    errors[agent.lastCompletedTurn] = detail?.error;
  const existing = new Set(
    items.flatMap((item) =>
      item.nativeNotice === "error" && item.turnId ? [item.turnId] : [],
    ),
  );

  const read =
    agent?.id && agent.threadId && requested.length && net
      ? net.reads.analytics({
          agent: agent.id,
          scope: AnalyticsScope.agent,
          view: "turn-errors",
          thread: agent.threadId,
          turns: requested.join(","),
          limit: 1,
        })
      : undefined;
  const current = read?.current;
  const analytics = current && resourceData(current);
  for (const turn of analytics?.turns || []) {
    if (
      turn.agentId === agent?.id &&
      turn.threadId === agent?.threadId &&
      turn.turnId &&
      requested.includes(turn.turnId) &&
      ["failed", "uncertain"].includes(turn.status || "") &&
      turn.error
    ) {
      const recovered = createTurnErrorMirror(turn).current.error;
      if (recovered && !errors[turn.turnId]) errors[turn.turnId] = recovered;
    }
  }

  const enriched = (() => {
    const last = new Map<string, number>();
    const unconfirmed = new Set(
      items
        .filter((item) => item.turnStatus === "uncertain")
        .map((item) => item.turnId),
    );
    items.forEach((item, index) => {
      if (item.turnId) last.set(item.turnId, index);
    });
    if (!Object.keys(errors).length) return items;
    return items.flatMap((item, index): Message[] => {
      const turnId = item.turnId;
      if (!turnId) return [item];
      const error = errors[turnId];
      if (!error || existing.has(turnId) || last.get(turnId) !== index)
        return [item];
      if (unconfirmed.has(turnId))
        return [
          {
            ...item,
            turnStatus: "uncertain",
            turnError: failureMessage(error),
          },
        ];
      return [
        item,
        {
          id: `turn-error:${agent?.id}:${agent?.threadId}:${turnId}`,
          role: "system",
          text: "",
          turnId,
          turnStatus: "failed",
          nativeNotice: "error",
          nativeError: error,
        },
      ];
    });
  })();
  return {
    items: enriched,
    loading:
      current && ["absent", "loading"].includes(current.status)
        ? requested
        : [],
    failed: current?.status === "error" ? requested : [],
    retry: (_turn: string) => {
      void read?.read().catch(() => {});
    },
  };
}
