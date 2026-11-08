import { Profiler, type ComponentProps } from "react";
import TurnHistory from "../../conversation/transcript/TurnHistory/TurnHistory.tsx";
import { collector } from "../../../perf/collector.ts";
export * from "../../conversation/transcript/TurnHistory/TurnHistory.tsx";
export default function ProfiledHistory(
  props: ComponentProps<typeof TurnHistory>,
) {
  return (
    <Profiler
      id="history"
      onRender={(id, _phase, duration) => collector.render(id, duration)}
    >
      <TurnHistory {...props} />
    </Profiler>
  );
}
