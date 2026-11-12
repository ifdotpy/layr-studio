import { appendFileSync } from "node:fs";
import { query as baseQuery } from "./fixtures/idle-compaction-sdk.ts";
import type {
  SessionFixtureTextQuery,
  SessionFixtureLegacyMessage,
} from "./fixtures/session-sdk-types.ts";
export {
  tool,
  createSdkMcpServer,
  getSessionMessages,
  resolveSettings,
  forkSession,
} from "./fixtures/idle-compaction-sdk.ts";
function trace(value: unknown) {
  appendFileSync("containment-trace", JSON.stringify(value) + "\n");
}
export function query(args: SessionFixtureTextQuery) {
  const q = baseQuery(args);
  if (args.options.persistSession === false) return q;
  trace({ query: true });
  let flagApplications = 0;
  let appliedModel = args.options.model;
  return {
    ...q,
    nativeReceiver: true,
    async setModel(this: { nativeReceiver: boolean }, model: string) {
      if (!this?.nativeReceiver) throw Error("setModel lost native receiver");
      trace({ model: model ?? null });
      if (
        process.env.FAIL_FEATURE === "model" ||
        (process.env.FAIL_FEATURE === "restore" && model === args.options.model)
      )
        throw new Error("setModel is not supported");
      appliedModel = model;
    },
    async applyFlagSettings(this: { nativeReceiver: boolean }, flags: unknown) {
      if (!this?.nativeReceiver)
        throw Error("applyFlagSettings lost native receiver");
      trace({ flags });
      if (
        process.env.FAIL_FEATURE === "flags" ||
        (process.env.FAIL_FEATURE === "flags-once" && flagApplications++ === 0)
      )
        throw new Error("applyFlagSettings is not supported");
    },
    setMcpServers: async (servers: Record<string, unknown>) => {
      trace({ servers: Object.keys(servers) });
      return { added: [], removed: ["studio"], errors: {} };
    },
    ...(process.env.FAIL_FEATURE === "missing"
      ? { setModel: undefined, applyFlagSettings: undefined }
      : {}),
    async *[Symbol.asyncIterator](): AsyncGenerator<SessionFixtureLegacyMessage> {
      for await (const message of q) {
        if (message.type === "user")
          trace({ input: message.uuid, actualModel: appliedModel });
        if (message.type === "result") {
          if (process.env.FAIL_FEATURE === "notice")
            yield {
              type: "system",
              subtype: "notification",
              priority: "high",
              // @ts-expect-error Runtime drift: known optional notice with incompatible text.
              text: { incompatible: true },
            };
          if (process.env.FAIL_FEATURE === "progress") {
            yield {
              type: "assistant",
              uuid: "tool-message",
              message: {
                id: "tool-response",
                role: "assistant",
                model: "claude-opus-5-5",
                content: [
                  {
                    type: "tool_use",
                    id: "progress-tool",
                    name: "Bash",
                    input: { command: "pwd" },
                  },
                ],
              },
            };
            yield {
              type: "tool_progress",
              tool_use_id: "progress-tool",
              // @ts-expect-error Runtime drift: progress duration has an incompatible type.
              elapsed_time_seconds: "invalid",
            };
          }
          if (process.env.FAIL_FEATURE === "diagnostics")
            for (let i = 0; i < 300; i++)
              // @ts-expect-error Runtime drift: undeclared informational subtypes.
              yield { type: "system", subtype: "future_" + i };
        }
        yield message;
      }
    },
  };
}
