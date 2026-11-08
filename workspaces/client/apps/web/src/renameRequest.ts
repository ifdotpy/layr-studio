import type { PostBody, PostResult } from "./api";

type RenameBody = PostBody<"/api/rename">;
type RenameReceipt = PostResult<"/api/rename">;

export async function submitRename(
  body: RenameBody,
  post: (path: "/api/rename", body: RenameBody) => Promise<RenameReceipt>,
  recover: () => Promise<unknown>,
): Promise<RenameReceipt> {
  try {
    // Success (including pending) is completed by the agent entity upsert.
    return await post("/api/rename", body);
  } catch (error) {
    // A lost write response permits one projection recovery, never another write.
    if (error instanceof TypeError) await recover().catch(() => {});
    throw error;
  }
}
