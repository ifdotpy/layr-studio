import { observer } from "mobx-react-lite";
import { Brain, Clock3 } from "lucide-react";
import { useMemo } from "react";
import { now } from "../../../../clock.ts";
import type { Message } from "../../../../types.ts";
import { useVisualActivity } from "../../../../hooks/useVisualActivity.ts";

export default observer(function ReasoningDuration({
  item,
}: {
  item: Pick<
    Message,
    | "id"
    | "sourceId"
    | "reasoningSince"
    | "reasoningObservedAt"
    | "reasoningMs"
    | "observedWait"
  >;
}) {
  const [visualRef, visualActive] = useVisualActivity<HTMLDivElement>();
  const since =
    typeof item.reasoningSince === "number" ? item.reasoningSince : undefined;
  const observedAt =
    typeof item.reasoningObservedAt === "number"
      ? item.reasoningObservedAt
      : undefined;
  const running = since !== undefined;
  const start = useMemo(() => Date.now(), [item.id, since, observedAt]);
  const elapsed =
    running && visualActive ? Math.max(0, now() - start) / 1000 : 0;
  const seconds = Math.max(
    0,
    Math.floor(
      (item.reasoningMs || 0) / 1000 +
        (running ? Math.max(0, (observedAt ?? since) - since) + elapsed : 0),
    ),
  );
  const duration =
    seconds < 60
      ? `${seconds}s`
      : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
  const Icon = item.observedWait ? Clock3 : Brain;
  return (
    <div
      ref={visualRef}
      className="reasoning-duration"
      data-message={item.id}
      data-source-message={item.sourceId}
      data-running={running}
    >
      <Icon size={14} aria-hidden="true" />
      <span>
        {running ? "Thinking" : item.observedWait ? "Waited" : "Thought"}{" "}
        <span className="reasoning-time">{duration}</span>
      </span>
    </div>
  );
});
