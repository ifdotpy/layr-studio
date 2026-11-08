import { ApiError, type PostResult } from "../api";
type CodeReceipt = Pick<
  PostResult<"/api/accounts/claude/login">,
  "status" | "codeSubmitted"
>;

export function codeVerificationPending(receipt: CodeReceipt | null): boolean {
  return receipt?.status === "pending" && receipt.codeSubmitted === true;
}

export function codeSubmissionResolved(receipt: CodeReceipt | null): boolean {
  return (
    receipt?.codeSubmitted === true ||
    ["ready", "error", "cancelled"].includes(receipt?.status || "")
  );
}

export function codeSubmissionNotApplied(error: unknown): boolean {
  return error instanceof ApiError && error.outcome === "not_applied";
}
