import { Profiler, type ComponentProps } from "react";
import TurnHistory from "../components/conversation/transcript/TurnHistory";
import { collector } from "./collector";
export * from "../components/conversation/transcript/TurnHistory";
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
