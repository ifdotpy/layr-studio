import { ApiError } from "./api.ts";
import { serverMutationResponseIsUncertain } from "./servers/mutationIdentity.ts";
import type { CommandResponse } from "./commands.ts";

/** Delete a chat through the command path. `finish` gets every id that the server deleted. */
export async function deleteConversation(
  remove: (id: string, signal: AbortSignal) => Promise<CommandResponse>,
  id: string,
  finish: (deleted: string[]) => void,
  signal: AbortSignal,
): Promise<CommandResponse> {
  const result = await remove(id, signal);
  finish(result.kind === "delete" ? result.result.deleted : [id]);
  return result;
}

export function definiteDeleteRejection(error: unknown): boolean {
  return (
    error instanceof ApiError &&
    error.status < 500 &&
    !error.outcome &&
    !serverMutationResponseIsUncertain(error.status, error.response)
  );
}
