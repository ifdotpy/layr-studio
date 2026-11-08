import { useServerStore } from "../servers/ServerServicesContext.tsx";
import { useMemo, useLayoutEffect, type RefObject } from "react";
import type { Agent, Message } from "../types.ts";
import {
  completed,
  type ChatReadProof,
} from "../app/conversation/ReadStateOwner.ts";
export {
  completed,
  matchingCompletedAgent,
  type ChatReadState,
  type ChatReadProof,
} from "../app/conversation/ReadStateOwner.ts";

// Transcript selection alone is not a read. Observe the final result in its scroll root.
export function useVisibleChatResult(
  scroll: RefObject<HTMLDivElement | null>,
  agent: Agent | undefined,
  items: Message[],
  loaded: boolean,
  onReadResult?: (proof: ChatReadProof) => void,
  workspace?: string,
  latestPage = false,
) {
  const state = useServerStore().state().current;
  const model =
    (state.status === "live" || state.status === "stale") && agent
      ? state.data.agents.get(agent.id)
      : undefined;
  const proof = model && completed(model);
  const finalId = useMemo(() => {
    const candidates = proof
      ? items.filter(
          (item) =>
            item.role === "assistant" &&
            item.turnId === proof.turnId &&
            (!item.phase || item.phase === "final_answer") &&
            !item.streamPending &&
            !item.streaming &&
            !!item.text?.trim(),
        )
      : [];
    return (
      candidates.filter((item) => item.phase === "final_answer").at(-1) ||
      candidates.at(-1)
    )?.id;
  }, [items, proof?.turnId]);
  const nativeTurnItemIds = useMemo(
    () =>
      proof
        ? items
            .filter(
              (item) =>
                item.turnId === proof.turnId &&
                item.role !== "user" &&
                !item.streamPending &&
                !item.streaming,
            )
            .map((item) => item.id)
        : [],
    [items, proof?.turnId],
  );
  const nativeTurnItemIdsKey = nativeTurnItemIds.join("\u0000");
  const completedTurnVisible =
    latestPage &&
    items.some(
      (item) =>
        item.turnId === proof?.turnId &&
        item.turnStatus === "completed" &&
        !item.streamPending &&
        !item.streaming,
    );
  useLayoutEffect(() => {
    if (
      !loaded ||
      !agent ||
      !model?.markRead ||
      !onReadResult ||
      !proof ||
      (!finalId && !completedTurnVisible)
    )
      return;
    const root = scroll.current;
    if (!root) return;
    let target: HTMLElement | undefined;
    let disposed = false;
    let reported = false;
    const visible = () => {
      if (
        disposed ||
        reported ||
        document.visibilityState !== "visible" ||
        !document.hasFocus() ||
        !target?.isConnected ||
        target.hasAttribute("data-lazy-message") ||
        (!finalId && !target.closest<HTMLElement>('[data-outcome="completed"]'))
      )
        return;
      const box = target.getBoundingClientRect();
      const bounds = root.getBoundingClientRect();
      if (
        Math.min(box.bottom, bounds.bottom, window.innerHeight) >
          Math.max(box.top, bounds.top, 0) &&
        Math.min(box.right, bounds.right, window.innerWidth) >
          Math.max(box.left, bounds.left, 0)
      ) {
        reported = true;
        onReadResult?.(proof);
      }
    };
    const observer = new IntersectionObserver(
      (entries) => {
        if (
          entries.some(
            (entry) => entry.target === target && !entry.isIntersecting,
          )
        )
          reported = false;
        if (
          entries.some(
            (entry) => entry.target === target && entry.isIntersecting,
          )
        )
          visible();
      },
      { root },
    );
    const connect = () => {
      const next = finalId
        ? (root.querySelector<HTMLElement>(
            `[data-message="${CSS.escape(finalId)}"]:not([data-lazy-message])`,
          ) ?? undefined)
        : (() => {
            for (
              let index = nativeTurnItemIds.length - 1;
              index >= 0;
              index--
            ) {
              const id = nativeTurnItemIds[index]!;
              const row = root.querySelector<HTMLElement>(
                `[data-message="${CSS.escape(id)}"]:not([data-lazy-message])`,
              );
              if (
                row?.isConnected &&
                row.getClientRects().length &&
                row.closest<HTMLElement>('[data-outcome="completed"]')
              )
                return row;
            }
            const singleTurn = root.querySelector<HTMLElement>(
              `[data-turn="${CSS.escape(proof.turnId)}"][data-outcome="completed"]`,
            );
            const nativeTurns = singleTurn?.dataset.turns
              ?.trim()
              .split(/\s+/)
              .filter(Boolean);
            return nativeTurns && nativeTurns.length > 1
              ? undefined
              : (singleTurn ?? undefined);
          })();
      if (target !== next) {
        observer.disconnect();
        target = next;
        reported = false;
        if (target) observer.observe(target);
      }
    };
    const resume = () => {
      reported = false;
      connect();
      visible();
    };
    const mutations = new MutationObserver((records) => {
      connect();
      if (
        records.some(
          (record) => record.type === "attributes" && record.target === target,
        )
      )
        visible();
    });
    mutations.observe(root, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["data-outcome", "open"],
    });
    connect();
    window.addEventListener("focus", resume);
    window.addEventListener("online", resume);
    document.addEventListener("visibilitychange", resume);
    return () => {
      disposed = true;
      observer.disconnect();
      mutations.disconnect();
      window.removeEventListener("focus", resume);
      window.removeEventListener("online", resume);
      document.removeEventListener("visibilitychange", resume);
    };
  }, [
    scroll,
    proof?.id,
    proof?.threadId,
    proof?.turnId,
    model?.markRead,
    finalId,
    nativeTurnItemIdsKey,
    completedTurnVisible,
    loaded,
    onReadResult,
    workspace,
  ]);
}
