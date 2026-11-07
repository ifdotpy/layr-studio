import { createObservableResources } from "../generated/observableResources.ts";
import { createObservableResourceRuntime } from "./runtime.ts";

/** The caller supplies its server's existing subscription module. */
export function createServerStore(
  subscriptions: Parameters<typeof createObservableResourceRuntime>[0],
) {
  const runtime = createObservableResourceRuntime(subscriptions);
  return { ...createObservableResources(runtime), dispose: runtime.dispose };
}
