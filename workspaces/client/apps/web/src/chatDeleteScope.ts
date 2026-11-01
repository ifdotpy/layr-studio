import { ApiError } from "./api";
import { serverMutationResponseIsUncertain } from "./servers/mutationIdentity";
import type { components } from "./generated/api";

export async function deleteConversation(
  post: (
    path: "/api/conversation/delete",
    body: { id: string },
    options: { signal: AbortSignal },
  ) => Promise<components["schemas"]["DeletedResponse"]>,
  id: string,
  finish: (deleted: string[]) => void,
  signal: AbortSignal,
): Promise<void> {
  const result = await post("/api/conversation/delete", { id }, { signal });
  finish(result.deleted);
}

export function definiteDeleteRejection(error: unknown): boolean {
  return (
    error instanceof ApiError &&
    error.status < 500 &&
    !error.outcome &&
    !serverMutationResponseIsUncertain(error.status, error.response)
  );
}
