import type { SessionFixtureMessage } from "./fixtures/session-sdk-types.ts";
import { contextUsage } from "./usage-fixtures.ts";
import { waitForFile } from "./sdk-fixtures.ts";
import fs from "node:fs/promises";
import path from "node:path";
export { resolveSettings } from "./sdk-fixtures.ts";
import type { FixtureQuery } from "./sdk-fixtures.ts";
export {
  tool,
  createSdkMcpServer,
  forkSession,
  getSessionMessages,
} from "./sdk-fixtures.ts";
// The parent test writes this private environment document from SDK-checked fixtures.
function readEvents(): SessionFixtureMessage[] {
  return JSON.parse(process.env.STUDIO_FIXTURE_EVENTS!);
}
const events = readEvents();
export function query({ prompt, options }: FixtureQuery) {
  const abort = new AbortController();
  let contextSample = 0;
  return {
    supportedModels: async () => [
      {
        value: "sonnet",
        displayName: "Sonnet",
        resolvedModel: "claude-sonnet-4-6",
      },
    ],
    accountInfo: async () => ({
      email: "test@example.test",
      subscriptionType: "Claude Max",
      apiProvider: "firstParty",
    }),
    initializationResult: async () => ({ commands: [] }),
    mcpServerStatus: async () => [],
    getContextUsage: async () => {
      const sample = ++contextSample;
      if (sample >= Number(process.env.STUDIO_DEFER_CONTEXT || Infinity)) {
        const file = path.join(options.cwd, `.context-${sample}`);
        process.stdout.write(
          JSON.stringify({
            method: "fixture/signal",
            params: { name: `context-pending-${sample}` },
          }) + "\n",
        );
        await waitForFile(file, { signal: abort.signal });
        const value = await fs.readFile(file, "utf8");
        process.stdout.write(
          JSON.stringify({
            method: "fixture/signal",
            params: { name: `context-settled-${sample}` },
          }) + "\n",
        );
        if (value === "reject") throw new Error("Deferred control unavailable");
        return { ...contextUsage, totalTokens: Number(value) };
      }
      if (process.env.STUDIO_CONTEXT_USAGE_FAILURE)
        throw new Error("Control unavailable");
      return contextUsage;
    },
    setModel: async () => {},
    setPermissionMode: async () => {},
    applyFlagSettings: async () => {},
    setMcpServers: async () => ({ added: [], removed: [], errors: {} }),
    close() {
      if (process.env.STUDIO_CONTEXT_CLOSE_WAIT) {
        process.stdout.write(
          JSON.stringify({
            method: "fixture/signal",
            params: { name: "query-closed" },
          }) + "\n",
        );
        return;
      }
      abort.abort();
    },
    interrupt: async () => abort.abort(),
    async *[Symbol.asyncIterator]() {
      while (!abort.signal.aborted) {
        const input = await prompt.next();
        if (input.done) return;
        yield { type: "user", isReplay: true, uuid: input.value.uuid };
        yield {
          type: "system",
          subtype: "init",
          session_id: options.resume || options.sessionId,
        };
        if (process.env.STUDIO_CONTEXT_CLOSE_WAIT) {
          await prompt.next(); // Consume a steer without acknowledging its delivery.
          yield {
            type: "result",
            subtype: "success",
            is_error: false,
            result: "Done",
            user_message_uuid: input.value.uuid,
          };
          await waitForFile(path.join(options.cwd, ".iterator-exit"));
          return;
        }
        for (const event of events) yield event;
      }
    },
  };
}
