import { serverSidebarBackend } from "../../servers/serverSidebarBackend.ts";
import { ServerAreasProvider } from "../../components/shell/ServerAreasProvider/ServerAreasProvider.tsx";
import { SidebarListOwner } from "../shell/SidebarOwner.ts";
import { ToastNotice } from "../../components/shell/ToastNotice/ToastNotice.tsx";
import { MantineProvider } from "@mantine/core";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment happy-dom
import { action, autorun, observable } from "mobx";
import type { ResourceStatus } from "../../store/runtime.ts";
import { afterEach, expect, it, vi } from "vitest";
import type { PostResult } from "../../api.ts";
import { ownerFixture } from "./ownerFixture.ts";
import { required } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});
function fixture() {
  const f = ownerFixture();
  type Receipt = PostResult<"/api/rename">;
  const slots = new Map<
    string,
    ReturnType<typeof observable.box<ResourceStatus<Receipt>>>
  >();
  vi.spyOn(f.services.store, "renameProgress").mockImplementation(
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
          return required(slot).get() ?? { status: "loading" };
        },
      };
    },
  );
  return {
    ...f,
    latest: () =>
      required(f.services.requests.latest(["workspace", "rename", "lead"])),
    accept: action((receipt: Receipt) => {
      slots
        .get(receipt.request_id || "")
        ?.set({ status: "live", data: receipt });
    }),
  };
}
it("SSE before HTTP and a late A reply cannot clear the newer rename", async () => {
  const f = fixture();
  let finish: ((receipt: PostResult<"/api/rename">) => void) | undefined;
  f.post.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  try {
    const first = f.rename.submit("lead", "/rename", null);
    const old = f.latest();
    f.accept({
      id: "lead",
      request_id: old.requestId,
      status: "applied",
      name: "first",
    });
    f.post.mockResolvedValue({ id: "lead", status: "pending" });
    await f.rename.submit("lead", "/rename", null);
    const newer = f.latest();
    required(finish)({
      id: "lead",
      request_id: old.requestId,
      status: "failed",
      error: "old failure",
    });
    await first;
    expect(f.latest()).toBe(newer);
    expect(f.notify).not.toHaveBeenCalledWith("old failure");
  } finally {
    f.dispose();
  }
});
it("explicit retry sends the saved slash rename and SSE after timeout settles it", async () => {
  const f = fixture();
  f.post.mockRejectedValue(new TypeError("reply lost"));
  try {
    await expect(f.rename.submit("lead", "/rename", null)).rejects.toThrow(
      "unknown",
    );
    const entry = f.latest();
    const stop = autorun(() => entry.view);
    await f.services.requests.retry(entry.requestId);
    expect(entry.view.errorText).toContain("unknown");
    expect(required(f.post.mock.calls[0])).toEqual(
      required(f.post.mock.calls[1]),
    );
    f.accept({
      id: "lead",
      request_id: entry.requestId,
      status: "applied",
      name: "Generated",
    });
    expect(entry.state).toBe("applied-from-SSE");
    stop();
  } finally {
    f.dispose();
  }
});
it("distinct equal-text actions get fresh identities", async () => {
  const f = fixture();
  f.post.mockResolvedValue({ id: "lead", status: "pending" });
  try {
    await f.rename.submit("lead", "/rename", null);
    const first = f.latest();
    await f.rename.submit("lead", "/rename", null);
    expect(f.latest().requestId).not.toBe(first.requestId);
  } finally {
    f.dispose();
  }
});
it("an unchanged old slash intent keeps its persisted id before the old record is removed", async () => {
  const f = fixture();
  const key = `studio-pending-renames:${f.areas.startup.creationScope}`;
  f.services.storage.setItem(
    key,
    JSON.stringify({ lead: { text: "/rename Saved", id: "old-id" } }),
  );
  f.post.mockRejectedValue(new TypeError("reply lost"));
  try {
    await expect(
      f.rename.submit("lead", "/rename Saved", "Saved"),
    ).rejects.toThrow("unknown");
    expect(f.latest().requestId).toBe("old-id");
    expect(f.services.storage.getItem(key)).toBeNull();
  } finally {
    f.dispose();
  }
});

it("a direct sidebar rename reports the registry unknown error", async () => {
  const f = fixture();
  const sidebar = new SidebarListOwner(f.areas);
  f.post.mockRejectedValueOnce(new TypeError("reply lost"));
  try {
    sidebar.name = "Manual";
    await sidebar.rename(new Event("submit"), "lead");
    const entry = f.latest();
    expect(entry.view.errorText).toContain("unknown");
    expect(f.notify).toHaveBeenCalledWith(entry.view.errorText);
  } finally {
    f.dispose();
  }
});

it("the rendered retry button dispatches the frozen rename rather than a fresh action", async () => {
  const errors = vi.spyOn(console, "error");
  const f = fixture();
  f.post.mockRejectedValueOnce(new TypeError("reply lost"));
  const host = document.createElement("div"),
    root = createRoot(host);
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  try {
    await expect(f.rename.rename("lead", "Saved")).rejects.toThrow("unknown");
    const entry = f.latest();
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ServerAreasProvider, {
            areas: f.areas,
            children: createElement(ToastNotice),
          }),
        ),
      ),
    );
    const button = required(host.querySelector("button"));
    expect(button.textContent).toContain("Retry saved request");
    f.post.mockImplementationOnce(async (_path, body) => ({
      ...body,
      status: "applied",
    }));
    await act(async () => button.click());
    expect(f.post).toHaveBeenCalledTimes(2);
    expect(f.post.mock.calls[1]).toEqual(f.post.mock.calls[0]);
    expect(entry.state).toBe("applied");
    expect(host.textContent).not.toContain("unknown");
    expect(f.services.requests.get(entry.requestId)).toBeUndefined();
    expect(errors.mock.calls.flat().map(String).join(" ")).not.toContain(
      "Cannot update a component",
    );
  } finally {
    await act(async () => root.unmount());
    f.dispose();
  }
});
it("retry storage failure is displayed without dispatching another rename", async () => {
  const f = fixture();
  f.post.mockRejectedValueOnce(new TypeError("reply lost"));
  const host = document.createElement("div"),
    root = createRoot(host);
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  try {
    await expect(f.rename.rename("lead", "Saved")).rejects.toThrow("unknown");
    const entry = f.latest();
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ServerAreasProvider, {
            areas: f.areas,
            children: createElement(ToastNotice),
          }),
        ),
      ),
    );
    vi.spyOn(f.services.requests.storage, "setItem").mockImplementationOnce(
      () => {
        throw new Error("Storage is full");
      },
    );
    await act(async () => required(host.querySelector("button")).click());
    expect(f.post).toHaveBeenCalledOnce();
    expect(f.notify).toHaveBeenCalledWith("Storage is full");
    expect(entry.state).toBe("unknown");
  } finally {
    await act(async () => root.unmount());
    f.dispose();
  }
});
it("rerendering the retry notice routes to the replacement server with the same agent id", async () => {
  const a = fixture(),
    b = fixture();
  a.post.mockRejectedValueOnce(new TypeError("reply lost"));
  b.post.mockRejectedValueOnce(new TypeError("reply lost"));
  const host = document.createElement("div"),
    root = createRoot(host);
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  try {
    await expect(a.rename.rename("lead", "First")).rejects.toThrow("unknown");
    await expect(b.rename.rename("lead", "Second")).rejects.toThrow("unknown");
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ServerAreasProvider, {
            areas: a.areas,
            children: createElement(ToastNotice),
          }),
        ),
      ),
    );
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ServerAreasProvider, {
            areas: b.areas,
            children: createElement(ToastNotice),
          }),
        ),
      ),
    );
    b.post.mockImplementationOnce(async (_path, body) => ({
      ...body,
      status: "applied",
    }));
    await act(async () => required(host.querySelector("button")).click());
    expect(a.post).toHaveBeenCalledOnce();
    expect(b.post).toHaveBeenCalledTimes(2);
    expect(b.post.mock.calls[1]).toEqual(b.post.mock.calls[0]);
  } finally {
    await act(async () => root.unmount());
    a.dispose();
    b.dispose();
  }
});
it("renaming an unopened server captures the API workspace established by ready", async () => {
  const f = fixture();
  f.post.mockImplementation(async (_path, body) => ({
    ...body,
    status: "applied",
  }));
  vi.spyOn(f.services, "ready").mockImplementation(async () => {
    f.services.api.setWorkspace("workspace");
  });
  try {
    const backend = serverSidebarBackend(f.services, () => true);
    await required(backend.rename)("lead", "Unopened server");
    expect(f.post.mock.calls[0]?.[2]).toMatchObject({
      workspaceId: "workspace",
    });
    expect(f.services.requests.forTarget("workspace", "lead")).toHaveLength(0);
  } finally {
    f.dispose();
  }
});
