// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { flowResult } from "mobx";
import { observer } from "mobx-react-lite";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, type PostBody } from "../../../api.ts";
import { outboxRequests } from "../../../browserPersistence.ts";
import { preferenceMemory } from "../../../servers/storage.ts";
import { localSidebarBackend, SidebarBackendProvider } from "./services.tsx";
import {
  ProjectSave,
  useProjectSave,
} from "../../projects/support/useProjectSave.ts";

const body: PostBody<"/api/peer-teams"> = {
  action: "move",
  path: "/work/project",
  member: "source-lead",
  team_id: "team-1",
  expected_revision: 4,
  request_id: "request-1",
};
const changed: PostBody<"/api/peer-teams"> = {
  ...body,
  request_id: "request-2",
  expected_revision: 5,
};
function fixture(storageKey?: string) {
  const backend = { ...localSidebarBackend, storage: preferenceMemory };
  const post = vi.spyOn(backend, "post").mockResolvedValue({});
  const saved = vi.fn(async () => {});
  return {
    post,
    backend,
    saved,
    save: new ProjectSave(backend, "/api/peer-teams", saved, storageKey),
  };
}
beforeEach(() => {
  preferenceMemory.clear();
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
});
afterEach(() => vi.unstubAllGlobals());

describe("ProjectSave", () => {
  it("locks synchronously and retries the identical request after a lost response", async () => {
    const f = fixture("project");
    const attempt = Promise.withResolvers<never>();
    f.post.mockReturnValueOnce(attempt.promise);
    const first = flowResult(f.save.submit(body));
    expect(f.save.pending).toBe(true);
    await flowResult(f.save.submit(changed));
    expect(f.post).toHaveBeenCalledTimes(1);
    attempt.reject(new TypeError("Lost response"));
    await first;
    expect(f.save).toMatchObject({
      pending: false,
      frozen: true,
      error: "Lost response",
      retryLabel: "Retry saved request",
    });
    await flowResult(f.save.submit(changed));
    expect(f.post.mock.calls[1]?.[1]).toBe(body);
    expect(f.saved).toHaveBeenCalledOnce();
    expect(
      outboxRequests(preferenceMemory).get("projectSave", "project"),
    ).toBeNull();
  });
  it("restores the durable request without replacing its identity or payload", async () => {
    const f = fixture("project");
    f.post.mockRejectedValueOnce(new TypeError("Offline"));
    await flowResult(f.save.submit(body));
    const restored = new ProjectSave(
      f.backend,
      "/api/peer-teams",
      f.saved,
      "project",
    );
    expect(restored.retryLabel).toBe("Retry saved request");
    await flowResult(restored.submit(changed));
    expect(f.post.mock.calls[1]?.[1]).toEqual(body);
  });
  it("releases a rejected request for a new edit but retains timeout uncertainty", async () => {
    const f = fixture("project");
    f.post.mockRejectedValueOnce(new ApiError("Conflict", 409));
    await flowResult(f.save.submit(body));
    expect(f.save).toMatchObject({
      frozen: false,
      request: null,
      retryLabel: null,
      error: "Conflict",
    });
    expect(
      outboxRequests(preferenceMemory).get("projectSave", "project"),
    ).toBeNull();
    f.post.mockRejectedValueOnce(new ApiError("Timed out", 408));
    await flowResult(f.save.submit(changed));
    expect(f.save).toMatchObject({ frozen: true, request: changed });
  });
  it("does not resend an acknowledged request if closing the form fails, including after remount", async () => {
    const f = fixture("project");
    f.saved.mockRejectedValueOnce(new Error("Close failed"));
    await flowResult(f.save.submit(body));
    expect(f.save.retryLabel).toBe("Refresh project");
    const restored = new ProjectSave(
      f.backend,
      "/api/peer-teams",
      f.saved,
      "project",
    );
    await flowResult(restored.submit(changed));
    expect(f.post).toHaveBeenCalledOnce();
    expect(f.saved).toHaveBeenCalledTimes(2);
  });
  it("does not send when the immutable request cannot be persisted", async () => {
    const f = fixture();
    const backend = {
      ...f.backend,
      storage: {
        ...preferenceMemory,
        setItem: () => {
          throw new Error("Storage full");
        },
      },
    };
    const save = new ProjectSave(
      backend,
      "/api/peer-teams",
      f.saved,
      "project",
    );
    await flowResult(save.submit(body));
    expect(save.error).toBe("Storage full");
    expect(save.frozen).toBe(true);
    expect(f.post).not.toHaveBeenCalled();
  });
  it("updates observer callers and keeps the connection chosen when the form opened", async () => {
    const f = fixture(),
      other = fixture();
    let current!: ReturnType<typeof useProjectSave<"/api/peer-teams">>;
    const Probe = observer(function Probe() {
      current = useProjectSave("/api/peer-teams", f.saved);
      return createElement(
        "output",
        null,
        current.pending ? "Saving" : current.error,
      );
    });
    const host = document.createElement("div"),
      root = createRoot(host);
    const render = (backend: typeof f.backend) =>
      createElement(
        SidebarBackendProvider,
        { value: backend },
        createElement(Probe),
      );
    try {
      await act(async () => root.render(render(f.backend)));
      await act(async () => root.render(render(other.backend)));
      const attempt = Promise.withResolvers<never>();
      f.post.mockReturnValueOnce(attempt.promise);
      let running!: Promise<void>;
      await act(async () => {
        running = flowResult(current.submit(body));
      });
      expect(host.textContent).toBe("Saving");
      await act(async () => {
        attempt.reject(new Error("Offline"));
        await running;
      });
      expect(host.textContent).toBe("Offline");
      expect(f.post).toHaveBeenCalledOnce();
      expect(other.post).not.toHaveBeenCalled();
    } finally {
      await act(async () => root.unmount());
    }
  });
});
