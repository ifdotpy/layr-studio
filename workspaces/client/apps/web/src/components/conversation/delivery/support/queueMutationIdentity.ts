import type { paths } from "../../../../generated/api.ts";

type QueueMutation =
  paths["/api/queue"]["post"]["requestBody"]["content"]["application/json"];

export function queueMutationMessageId(
  mutation: QueueMutation,
): string | undefined {
  if (mutation.action === "reorder") return undefined;
  return mutation.id || undefined;
}
