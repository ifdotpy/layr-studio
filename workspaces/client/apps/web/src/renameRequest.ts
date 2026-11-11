import { reaction } from "mobx";
import { z } from "zod";
import { agentId, requestId } from "./generated/observableResources.ts";
import { resourceData } from "./store/runtime.ts";
import { requestOperation } from "./servers/RequestRegistry.ts";

/** Manual HTTP receipts settle immediately; generated titles await exact SSE proof. */
export const renameOperation = requestOperation({
  path: "/api/rename",
  body: z
    .object({ id: z.string(), name: z.string().nullable().optional() })
    .strict(),
  label: (body) => `Rename ${body.name || body.id}`,
  async send({ api }, body, entry) {
    const reply = await api.post(
      "/api/rename",
      { ...body, request_id: entry.requestId },
      { requestId: entry.requestId, workspaceId: entry.key[0] },
    );
    if (body.name == null) return;
    if (
      reply.id !== body.id ||
      reply.request_id !== entry.requestId ||
      reply.status !== "applied"
    )
      throw new TypeError("The rename reply did not confirm this request.");
    return "applied";
  },
  watch({ store, notify }, body, entry, observe) {
    if (body.name != null) return () => {};
    return reaction(
      () =>
        resourceData(
          store().renameProgress(agentId(body.id), requestId(entry.requestId))
            .current,
        ),
      (receipt) => {
        if (
          !receipt ||
          receipt.id !== body.id ||
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
});
