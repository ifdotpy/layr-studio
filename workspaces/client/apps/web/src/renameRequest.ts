import { reaction } from "mobx";
import { agentId, requestId } from "./generated/observableResources.ts";
import { resourceData } from "./store/runtime.ts";
import type { createServerStore } from "./store/server.ts";
import { RequestRegistry } from "./servers/RequestRegistry.ts";
import type { post } from "./api.ts";

/** Manual HTTP receipts settle immediately; generated titles await exact SSE proof. */
export function createRequestRegistry(
  storage: Storage,
  api: { post: typeof post<"/api/rename"> },
  store: () => ReturnType<typeof createServerStore>,
  notify: (message: string) => void,
) {
  return new RequestRegistry(
    storage,
    async (entry): Promise<"applied" | void> => {
      const reply = await api.post(
        entry.path,
        { ...entry.body, request_id: entry.requestId },
        { requestId: entry.requestId, workspaceId: entry.key[0] },
      );
      if (entry.body.name != null) {
        if (
          reply.id !== entry.body.id ||
          reply.request_id !== entry.requestId ||
          reply.status !== "applied"
        )
          throw new TypeError("The rename reply did not confirm this request.");
        return "applied";
      }
    },
    (entry, observe) => {
      if (entry.body.name != null) return () => {};
      return reaction(
        () =>
          resourceData(
            store().renameProgress(
              agentId(entry.body.id),
              requestId(entry.requestId),
            ).current,
          ),
        (receipt) => {
          if (
            !receipt ||
            receipt.id !== entry.body.id ||
            receipt.request_id !== entry.requestId ||
            (receipt.status !== "applied" && receipt.status !== "failed")
          )
            return;
          if (receipt.status === "applied") observe({ kind: "applied" });
          else {
            const errorText =
              receipt.error || "The title could not be generated.";
            observe({ kind: "failed", errorText });
            notify(errorText);
          }
        },
        { fireImmediately: true },
      );
    },
  );
}
