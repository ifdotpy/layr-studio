import type { PostBody, PostResult } from "./api.ts";

type RenameBody = PostBody<"/api/rename">;
type RenameReceipt = PostResult<"/api/rename">;

export async function submitRename(
  body: RenameBody,
  post: (path: "/api/rename", body: RenameBody) => Promise<RenameReceipt>,
): Promise<RenameReceipt> {
  // The entity stream reconciles successful and uncertain write responses.
  return post("/api/rename", body);
}
