import { expect, it, vi } from "vitest";
const calls = vi.hoisted(() => ({
  remove: vi.fn(async () => {}),
  conditional: vi.fn(async () => {}),
}));
vi.mock("../../unsent/store", () => ({
  unsentStore: () => ({
    delete: calls.remove,
    deleteIfEqual: calls.conditional,
  }),
}));
import { removeAnswerDraft, requestDraftsForStorage } from "./requestDrafts";

it("keeps matching request IDs and secret answers with their storage owner", () => {
  const first = {} as Storage;
  const second = {} as Storage;
  const a = requestDraftsForStorage(first);
  const b = requestDraftsForStorage(second);
  a.set("same-request", { secret: "first secret" });
  b.set("same-request", { secret: "second secret" });
  expect(requestDraftsForStorage(first).get("same-request")).toEqual({
    secret: "first secret",
  });
  a.delete("same-request");
  expect(b.get("same-request")).toEqual({ secret: "second secret" });
  expect(requestDraftsForStorage({} as Storage).size).toBe(0);
});

it("clears a confirmed deleted question from memory and the buffer", async () => {
  const storage = {} as Storage;
  requestDraftsForStorage(storage).set("question", { answer: "pending" });
  await removeAnswerDraft(storage, "server", "question");
  expect(requestDraftsForStorage(storage).has("question")).toBe(false);
  expect(calls.remove).toHaveBeenCalledWith("question");
});
