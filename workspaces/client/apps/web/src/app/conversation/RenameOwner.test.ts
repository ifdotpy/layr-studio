// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import type { PostResult } from "../../api.ts";
const watches = vi.hoisted(
  () =>
    [] as {
      request: string;
      accept: (receipt: PostResult<"/api/rename">) => void;
    }[],
);
vi.mock("../../renameRequest.ts", async (original) => ({
  ...(await original<typeof import("../../renameRequest.ts")>()),
  watchRenameResult: (
    ...[, , request, accept]: Parameters<
      typeof import("../../renameRequest.ts").watchRenameResult
    >
  ) => {
    watches.push({ request, accept });
    return () => {};
  },
}));
import { ownerFixture } from "./ownerFixture.ts";
import { required } from "../../components/componentTestFixtures.ts";
afterEach(() => {
  watches.length = 0;
  vi.restoreAllMocks();
});
it("a completed receipt and late HTTP reply cannot clear the newer rename", async () => {
  const f = ownerFixture();
  let finish: ((receipt: PostResult<"/api/rename">) => void) | undefined;
  f.post.mockReturnValueOnce(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  const stop = f.rename.start();
  try {
    const first = f.rename.submit("lead", "/rename first", "first");
    const old = required(watches[0]);
    old.accept({
      id: "lead",
      request_id: old.request,
      status: "applied",
      name: "first",
    });
    f.post.mockImplementation(async (_path, body) => ({
      id: body.id,
      request_id: body.request_id,
      status: "pending",
    }));
    await f.rename.submit("lead", "/rename newer", "newer");
    const newer = required(f.rename.pending.get("lead"));
    required(finish)({
      id: "lead",
      request_id: old.request,
      status: "failed",
      error: "old failure",
    });
    await first;
    old.accept({
      id: "lead",
      request_id: old.request,
      status: "applied",
      name: "first",
    });
    expect(f.rename.pending.get("lead")).toEqual(newer);
    expect(f.notify).not.toHaveBeenCalledWith("old failure");
  } finally {
    stop();
    f.dispose();
  }
});
it("retries the same slash rename with its saved identity and retains uncertainty on disposal", async () => {
  const f = ownerFixture();
  f.post.mockRejectedValue(new TypeError("reply lost"));
  const stop = f.rename.start();
  try {
    await expect(f.rename.submit("lead", "/rename", null)).rejects.toThrow(
      "reply lost",
    );
    await expect(f.rename.submit("lead", "/rename", null)).rejects.toThrow(
      "reply lost",
    );
    expect(required(f.post.mock.calls[0])[1]).toEqual(
      required(f.post.mock.calls[1])[1],
    );
    stop();
    expect(f.rename.pending.size).toBe(1);
  } finally {
    stop();
    f.dispose();
  }
});
