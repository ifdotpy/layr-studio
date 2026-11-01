import { describe, expect, it, vi } from "vitest";
import { ApiError } from "./api";
import { definiteDeleteRejection, deleteConversation } from "./chatDeleteScope";

describe("conversation deletion", () => {
  it("uses all server-deleted IDs when a worker was absent from the local list", async () => {
    const post = vi.fn().mockResolvedValue({ deleted: ["lead", "new-worker"] });
    const finish = vi.fn();
    const signal = new AbortController().signal;
    await deleteConversation(post, "lead", finish, signal);
    expect(post).toHaveBeenCalledOnce();
    expect(post).toHaveBeenCalledWith(
      "/api/conversation/delete",
      {
        id: "lead",
      },
      { signal },
    );
    expect(finish).toHaveBeenCalledWith(["lead", "new-worker"]);
  });

  it("does not resend a delete when its response is lost", async () => {
    const post = vi.fn().mockRejectedValue(new TypeError("response lost"));
    const finish = vi.fn();
    await expect(
      deleteConversation(post, "lead", finish, new AbortController().signal),
    ).rejects.toThrow("response lost");
    expect(post).toHaveBeenCalledOnce();
    expect(finish).not.toHaveBeenCalled();
  });

  it("distinguishes definite rejections from uncertain outcomes", () => {
    expect(
      definiteDeleteRejection(new ApiError({ code: "not_found" }, 404)),
    ).toBe(true);
    expect(
      definiteDeleteRejection(new ApiError({ code: "request_pending" }, 409)),
    ).toBe(false);
    expect(
      definiteDeleteRejection(new ApiError({ code: "timeout" }, 408)),
    ).toBe(false);
    expect(definiteDeleteRejection(new TypeError("response lost"))).toBe(false);
  });
});
