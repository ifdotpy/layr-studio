import { observable, runInAction } from "mobx";
import type { ResourceStatus } from "../../store/runtime.ts";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import type { PostResult } from "../../api.ts";
import { ownerFixture } from "./ownerFixture.ts";
import { required } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
afterEach(() => {
  vi.restoreAllMocks();
});
it("a completed receipt and late HTTP reply cannot clear the newer rename", async () => {
  const f = ownerFixture();
  type Receipt = PostResult<"/api/rename">;
  const slots = new Map<
    string,
    ReturnType<typeof observable.box<ResourceStatus<Receipt>>>
  >();
  vi.spyOn(f.areas.scope.store, "renameProgress").mockImplementation(
    (_agent, request) => {
      let slot = slots.get(request);
      if (!slot) {
        slot = observable.box<ResourceStatus<Receipt>>(
          { status: "loading" },
          { deep: false },
        );
        slots.set(request, slot);
      }
      return {
        get current() {
          return slot!.get() ?? { status: "loading" };
        },
      };
    },
  );
  let finish: ((receipt: PostResult<"/api/rename">) => void) | undefined;
  f.post.mockReturnValueOnce(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  const stop = f.rename.start();
  try {
    const first = f.rename.submit("lead", "/rename first", "first");
    const request = required(f.rename.pending.get("lead")).id;
    const old = {
      request,
      accept(receipt: Receipt) {
        runInAction(() =>
          slots.get(request)?.set({ status: "live", data: receipt }),
        );
      },
    };
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
