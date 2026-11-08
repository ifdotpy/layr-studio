import { describe, expect, it, vi } from "vitest";
import { ApiError } from "./api.ts";
import {
  definiteDeleteRejection,
  deleteConversation,
} from "./chatDeleteScope.ts";

const applied = (deleted: string[]) => ({
  status: "applied" as const,
  ref_kind: "agent",
  ref_id: "lead",
  rev: 0,
  seq: 1,
  rows: [],
  kind: "delete" as const,
  result: { deleted, stopped: "confirmed" as const },
});

describe("conversation deletion", () => {
  it("uses all server-deleted IDs when a worker was absent from the local list", async () => {
    const remove = vi.fn().mockResolvedValue(applied(["lead", "new-worker"]));
    const finish = vi.fn();
    const signal = new AbortController().signal;
    await deleteConversation(remove, "lead", finish, signal);
    expect(remove).toHaveBeenCalledOnce();
    expect(remove).toHaveBeenCalledWith("lead", signal);
    expect(finish).toHaveBeenCalledWith(["lead", "new-worker"]);
  });

  it("returns the server's unconfirmed stop outcome after removing the chats", async () => {
    const response = {
      ...applied(["lead"]),
      result: { deleted: ["lead"], stopped: "unconfirmed" as const },
    };
    const finish = vi.fn();
    const result = await deleteConversation(
      vi.fn().mockResolvedValue(response),
      "lead",
      finish,
      new AbortController().signal,
    );
    expect(result).toEqual(response);
    expect(finish).toHaveBeenCalledWith(["lead"]);
  });

  it("does not resend a delete when its response is lost", async () => {
    const remove = vi.fn().mockRejectedValue(new TypeError("response lost"));
    const finish = vi.fn();
    await expect(
      deleteConversation(remove, "lead", finish, new AbortController().signal),
    ).rejects.toThrow("response lost");
    expect(remove).toHaveBeenCalledOnce();
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
