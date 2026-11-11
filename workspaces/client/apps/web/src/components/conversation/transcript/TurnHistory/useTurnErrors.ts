import { resourceData } from "../../../../store/runtime.ts";
import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { agentId as resourceAgentId } from "../../../../generated/observableResources.ts";
import { computed } from "mobx";
import { createTurnErrorMirror } from "../../../../generated/observableResources.ts";
import {
  AnalyticsScope,
  AgentStatus as GeneratedApiAgentStatus,
} from "../../../../generated/api.ts";
import type { DeepReadonly } from "../../../../deepReadonly.ts";
import { useServerNet } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useMemo } from "react";
import "../../../../api.ts";
import type { Agent, JsonInput, Message } from "../../../../types.ts";
import { failureMessage } from "../../support/turnFailureReason.ts";

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
export function useTurnErrors(items: Message[], agent?: Agent, workspace = "") {
  const store = useServerStore();
  const detail = agent
    ? resourceData(store.agentDetail(resourceAgentId(agent.id)).current)
    : undefined;
  const net = useServerNet();
  const {
    errors: knownErrors,
    resolved,
    existing,
    turns,
  } = useMemo(
    () =>
      computed(() => {
        const errors: Record<string, DeepReadonly<JsonInput>> = {};
        const resolved = new Set<string>();
        for (const item of items) {
          if (item.turnErrorResolved && item.turnId) resolved.add(item.turnId);
          if (item.turnError && item.turnId)
            errors[item.turnId] = item.turnError;
        }
        const latest = detail?.nativeTurnError;
        if (latest?.turnId && latest.error)
          errors[latest.turnId] = latest.error;
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
        const turns = failedTurnsNeedingLookup(items, existing);
        return { errors, resolved, existing, turns };
      }),
    [
      items,
      detail?.nativeTurnError,
      agent?.status,
      agent?.turnId,
      agent?.lastCompletedTurn,
      detail?.error,
    ],
  ).get();
  const errors = { ...knownErrors };
  // Only the opening history needs the legacy fallback; live turns carry errors.
  const requested = useMemo(
    () =>
      turns.filter((turn) => !errors[turn] && !resolved.has(turn)).slice(-120),
    [store, workspace, agent?.id, agent?.threadId],
  );
  const read =
    agent?.id && agent.threadId && requested.length
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
  const enriched = useMemo(
    () =>
      computed(() => {
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
      }),
    [items, errors, existing, agent?.id, agent?.threadId],
  ).get();
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
