import { watchResourceReads } from "../sync/subscriptions/reads.ts";
import { createObservableResources } from "../generated/observableResources.ts";
import { createObservableResourceRuntime } from "./runtime.ts";
import { flatResourceKey } from "../sync/subscriptions/contracts.ts";
import type { ApiGetPath, GetOptions, GetResult, ServerApi } from "../api.ts";
export type SkillsReadContext = {
  workspace: string;
  account: string;
  cwd: string;
  provider: string;
};

/** The caller supplies its server's existing subscription module. */
export function createServerStore(
  subscriptions: Parameters<typeof createObservableResourceRuntime>[0],
  watch: typeof watchResourceReads = watchResourceReads,
) {
  const runtime = createObservableResourceRuntime(subscriptions);
  const read = <P extends ApiGetPath>(path: P) => {
    const keyed = runtime.keyed<GetResult<P>>({
      read: true,
      releaseOnUnobserved: true,
    });
    return (
      api: Pick<ServerApi, "get" | "getStreamSession">,
      query: NonNullable<GetOptions<P>["query"]>,
      context?: SkillsReadContext,
    ) => {
      const session = api.getStreamSession();
      return keyed({
        key: flatResourceKey({
          ...query,
          "@skills": context ? flatResourceKey(context) : undefined,
          "@workspace": session?.workspaceId,
          "@epoch": session?.sessionId,
          "@operation": path,
        }),
        ...(path === "/api/skills" && context
          ? {
              watch: (read: () => Promise<void>) =>
                watch(
                  { kind: "native-catalog", accountKey: context.account },
                  read,
                  () => {},
                  { waitForBaseline: true },
                ),
            }
          : {}),
        async read() {
          const data = await api.get(path, { query, readMetadata: {} });
          if (data === undefined)
            throw new Error("The server returned no read result.");
          return data;
        },
      });
    };
  };
  const reads = {
    file: read("/api/file"),
    analytics: read("/api/analytics"),
    search: read("/api/search"),
    directories: read("/api/directories"),
    projectLocations: read("/api/project-locations"),
    skills: read("/api/skills"),
    transcriptPage: read("/api/transcript/page"),
    transcriptSearch: read("/api/transcript/search"),
  };
  return {
    ...createObservableResources(runtime),
    reads,
    dispose: runtime.dispose,
  };
}
