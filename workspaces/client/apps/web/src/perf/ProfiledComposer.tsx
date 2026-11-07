import { Profiler, type ComponentProps } from "react";
import PromptComposer from "../components/prompt-composer/PromptComposer";
import { collector } from "./collector";
export * from "../components/prompt-composer/PromptComposer";
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
