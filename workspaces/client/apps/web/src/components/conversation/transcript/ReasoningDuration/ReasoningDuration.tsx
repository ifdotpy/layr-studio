import { ownedView } from "../support/ownedView.tsx";
import { makeAutoObservable } from "mobx";
import { derived } from "../../support/derived.ts";
import { Brain, Clock3 } from "lucide-react";
import { now } from "../../../../clock.ts";
import type { Message } from "../../../../types.ts";
import { useVisualActivity } from "../../../../hooks/useVisualActivity.ts";

type ReasoningItem = Pick<
  Message,
  | "id"
  | "sourceId"
  | "reasoningSince"
  | "reasoningObservedAt"
  | "reasoningMs"
  | "observedWait"
>;
class ReasoningStore {
  start = Date.now();
  time = derived((item: ReasoningItem, active: boolean) => {
    const since = item.reasoningSince ?? undefined;
    const observedAt = item.reasoningObservedAt ?? undefined;
    const running = since !== undefined;
    const elapsed =
      running && active ? Math.max(0, now() - this.start) / 1000 : 0;
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

    return { running, duration };
  });
  constructor() {
    makeAutoObservable(this, { start: false, time: false });
  }
}

export default ownedView(
  (_props: { item: ReasoningItem }) => new ReasoningStore(),
  function ReasoningDuration({ item, model }) {
    const [visualRef, visualActive] = useVisualActivity<HTMLDivElement>();
    const { running, duration } = model.time(item, visualActive);
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
  },
  (props) =>
    JSON.stringify([
      props.item.id,
      props.item.reasoningSince,
      props.item.reasoningObservedAt,
    ]),
);
