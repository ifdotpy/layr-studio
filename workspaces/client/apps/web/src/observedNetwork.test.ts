import { serverLocalStorage, serverSessionStorage } from "./servers/storage.ts";
// @vitest-environment happy-dom
import { autorun, spy } from "mobx";
import { expect, it, vi } from "vitest";
import { createApiScope } from "./servers/net/api.ts";
import { createMutationJournal } from "./servers/mutationIdentity.ts";
import { NetStatus } from "./servers/net/status.ts";
import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "./generated/apiSchema.ts";
import { SignInPush } from "./components/accounts/SignInPushSettings/SignInPush.ts";
import { ClaudeFileBackupsStore } from "./components/conversation/ClaudeFileBackups/ClaudeFileBackupsStore.ts";

it.each(["push", "backups"] as const)(
  "%s observation never writes request status synchronously",
  async (kind) => {
    const status = new NetStatus();
    const pending = Promise.withResolvers<Response>();
    const api = createApiScope({
      origin: "https://observation.test",
      fetch: vi.fn(() => pending.promise),
      localStorage: serverLocalStorage,
      sessionStorage: serverSessionStorage,
      beginMutation: createMutationJournal("observation", serverLocalStorage),
      requestStatus: status,
    });
    const model =
      kind === "push"
        ? new SignInPush(api)
        : new ClaudeFileBackupsStore(api, "agent");
    const read = () => ("state" in model ? model.state : model.usage);
    const writes: string[] = [];
    const stopSpy = spy((event) => {
      if (
        ["update", "add", "delete", "splice"].includes(event.type) &&
        !(event.type === "update" && event.observableKind === "computed")
      )
        writes.push(event.type);
    });
    const stop = autorun(read);
    stopSpy();
    try {
      expect(writes).toEqual([]);
      expect(status.inFlight).toBe(0);
      await Promise.resolve();
      expect(status.inFlight).toBe(1);
    } finally {
      stop();
      pending.resolve(
        new Response(JSON.stringify({ subscriptions: [] }), {
          headers: {
            "Content-Type": "application/json",
            [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
          },
        }),
      );
      await vi.waitFor(() => expect(status.inFlight).toBe(0));
    }
  },
);
