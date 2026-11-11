import { useServerTokenRates } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useCallback, useLayoutEffect, useRef } from "react";
import type { Agent } from "../../../types.ts";
import { formatTokenRate, tweenTokenRate } from "../../../usage/tokenRate.ts";
import { useVisualActivity } from "../../../hooks/useVisualActivity.ts";
import { observer } from "mobx-react-lite";

export default observer(function TokenRate({
  agent,
  variant = "footer",
}: {
  agent: Agent;
  variant?: "footer" | "worker";
}) {
  const { getTokenRate, getWorkspaceTeamTokenRate } = useServerTokenRates();
  const label = useRef<HTMLSpanElement | null>(null);
  const [visualRef, visualActive] = useVisualActivity<HTMLElement>();
  const observeRate = useCallback(
    (element: HTMLSpanElement | null) => {
      label.current = element;
      // Empty worker meters are display:none. Their visible metadata row must
      // keep the subscription active until the first rate arrives.
      visualRef(
        element && variant === "worker"
          ? element.parentElement || element
          : element,
      );
    },
    [visualRef, variant],
  );
  const scope = agent.id;
  const workspaceTeamRate =
    visualActive && variant === "worker"
      ? getWorkspaceTeamTokenRate(agent.rootId || "", agent.id)
      : null;
  const current = useRef(0);
  const previousTurn = useRef("");
  const rate = visualActive
    ? variant === "worker"
      ? workspaceTeamRate
      : getTokenRate(scope)
    : null;
  // Card metadata can lag the transcript. The batch owns its current turn.
  const matchesTurn =
    variant === "worker" || !agent.turnId || rate?.turnId === agent.turnId;
  const active = Boolean(rate?.active);
  const visible =
    matchesTurn &&
    rate !== null &&
    rate.outputTokens > 0 &&
    (!active || rate.rate > 0);
  const target = visible ? rate.rate : 0;
  const turn = `${agent.id}:${variant === "worker" ? rate?.turnId || "" : agent.turnId || rate?.turnId || ""}`;
  const prefix = rate?.estimated ? "≈" : "";
  useLayoutEffect(() => {
    const element = label.current;
    if (!visualActive || !element) return;
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    let frame = 0;
    const paint = (value: number) => {
      current.current = value;
      element.textContent = visible
        ? `${prefix}${formatTokenRate(value)} tok/s`
        : "";
      element.dataset.reducedMotion = String(media.matches);
    };
    const snap = () => {
      cancelAnimationFrame(frame);
      paint(target);
    };
    media.addEventListener("change", snap);
    // The first sample of a new turn must not tween from the previous turn.
    if (
      previousTurn.current !== turn ||
      media.matches ||
      !visible ||
      !current.current
    ) {
      previousTurn.current = turn;
      paint(target);
    } else {
      const from = current.current;
      paint(from);
      const started = performance.now();
      const tick = (now: number) => {
        const value = tweenTokenRate(from, target, now - started);
        paint(value);
        if (now - started < 450) frame = requestAnimationFrame(tick);
      };
      frame = requestAnimationFrame(tick);
    }
    return () => {
      cancelAnimationFrame(frame);
      media.removeEventListener("change", snap);
    };
  }, [target, turn, visible, visualActive, prefix]);
  return (
    <span
      ref={observeRate}
      data-visual-active={visualActive}
      className="token-rate"
      data-testid="token-rate"
      data-variant={variant}
      data-agent={agent.id}
      data-active={active}
      data-estimated={rate?.estimated || false}
      data-rate={visible ? target : ""}
      aria-label={
        visible
          ? `${rate?.estimated ? "Estimated " : ""}output tokens per second${active ? "" : ", last turn"}`
          : undefined
      }
      title={
        visible
          ? `${rate?.estimated ? "Estimate from recent text. Hidden reasoning can change the provider rate." : "Provider output tokens, including reasoning, over the observed response interval. Tool time is excluded."}${active ? "" : " Last turn."}`
          : undefined
      }
    >
      {visible ? `${prefix}${formatTokenRate(target)} tok/s` : ""}
    </span>
  );
});
