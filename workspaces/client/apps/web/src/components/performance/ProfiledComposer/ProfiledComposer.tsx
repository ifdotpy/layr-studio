import { Profiler, type ComponentProps } from "react";
import PromptComposer from "../../conversation/prompt-composer/PromptComposer/PromptComposer.tsx";
import { collector } from "../../../perf/collector.ts";
export * from "../../conversation/prompt-composer/PromptComposer/PromptComposer.tsx";
export default function ProfiledComposer(
  props: ComponentProps<typeof PromptComposer>,
) {
  return (
    <Profiler
      id="composer"
      onRender={(id, _phase, duration) => collector.render(id, duration)}
    >
      <PromptComposer {...props} />
    </Profiler>
  );
}
