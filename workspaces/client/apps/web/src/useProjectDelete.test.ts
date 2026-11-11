import type { ComponentSnapshot as Snapshot } from "./components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { observable, runInAction } from "mobx";
import {
  createStateMirror,
  createProjectionMetadataMirror,
  serializeStateCollections,
} from "./generated/observableResources.ts";
import { stateFromFixture } from "./servers/sidebarSourceFixture.ts";
import { componentSnapshot as snapshotFixture } from "./components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { writePreferenceEdit } from "./sync/uiPreferenceStore.ts";
import { StopOutcome } from "./generated/api.ts";
import { required } from "./components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { afterEach, expect, it, vi } from "vitest";
import type {
  SidebarServices,
  SidebarBackend,
} from "./components/sidebar/support/services.tsx";

import { ApiError } from "./api.ts";

vi.mock("./app/ServerAreasProvider.tsx", () => ({ useServerAreas: vi.fn() }));
import { ProjectDeletion } from "./useProjectDelete.ts";

const stops: (() => void)[] = [];
afterEach(() => stops.splice(0).forEach((stop) => stop()));
const project = {
  id: "project",
  path: "/project",
  name: "Example",
  homeServerId: "home",
};
const data: Snapshot = {
  ...Object.assign(
    snapshotFixture(
      [
        {
          id: "lead",
          cwd: "/project",
          projectId: "project",
          projectServerId: "home",
        },
      ],
      { projects: [project], projectSequences: { project: 12 }, rooms: [] },
    ),
    { token: "", stateDir: "/fixture" },
  ),
  stateDir: "fixture",
  entityWorkspaceId: "workspace",
  entitiesComplete: true,
};

function fixture() {
  const values = new Map<string, string>();
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
    removeItem: (key: string) => {
      values.delete(key);
    },
  };
  const post = vi.fn().mockImplementation(async (_path, body) => ({
    id: body.id,
    request_id: body.request_id,
    outcome: "deleted",
    stopped: StopOutcome.confirmed,
    deletedAgentIds: ["lead"],
    deletedRoomIds: [],
  }));
  const backend: SidebarBackend = {
    post,
    storage,
    ownerId: "home",
    saved: (key, fallback) => {
      const raw = storage.getItem(key);
      return raw === null ? fallback : JSON.parse(raw);
    },
    save: (key, value) => storage.setItem(key, JSON.stringify(value)),
    editPreference: writePreferenceEdit,
  };
  const owner = vi.fn(() => backend);
  const options = observable(
    {
      data,
      workspaceId: "workspace",
      services: {
        owner,
        cache: backend,
        byOwner: () => backend,
        serverForChat: () => backend.ownerId,
      } satisfies SidebarServices,
      onSuspend: vi.fn(),
      onResume: vi.fn(),
      onFinish: vi.fn(),
      notify: vi.fn(),
    },
    {},
    { deep: false },
  );
  const state = createStateMirror();
  const metadata = createProjectionMetadataMirror({
    workspaceId: "",
    ready: false,
    complete: false,
    projectSequences: {},
  });
  const result = new ProjectDeletion({
    ...options,
    get state() {
      return state.current;
    },
    get metadata() {
      return metadata.current;
    },
    get workspaceId() {
      return options.workspaceId;
    },
  });
  const stop = result.start();
  stops.push(stop);
  const render = () => {
    runInAction(() => {
      metadata.apply({
        workspaceId: options.data.entityWorkspaceId ?? "",
        ready: true,
        complete: options.data.entitiesComplete === true,
        projectSequences: options.data.runtime.projectSequences ?? {},
      });
      state.apply(serializeStateCollections(stateFromFixture(options.data)));
    });
    return result;
  };
  const settle = async () => {
    for (let i = 0; i < 5; i++) {
      await Promise.resolve();
    }
    return result;
  };
  return { options, render, settle, post, owner, storage, values, close: stop };
}

it("cancel posts nothing; repeated confirmation and effect replay send once to services.owner", async () => {
  const f = fixture();
  f.render().open("/project");
  f.render().dialog.onCancel();
  expect(f.render().dialog.name).toBeNull();
  expect(f.post).not.toHaveBeenCalled();
  f.render().open("project");
  const confirming = f.render();
  confirming.dialog.onConfirm();
  confirming.dialog.onConfirm();
  expect(f.options.onSuspend).toHaveBeenCalledOnce();
  expect(f.post).not.toHaveBeenCalled();
  f.render();
  await f.settle();
  expect(f.owner).toHaveBeenCalledWith({ kind: "project", path: "/project" });
  expect(f.post).toHaveBeenCalledOnce();

  expect(required(f.post.mock.calls[0])[1]).toMatchObject({
    id: "project",
    expected_project_seq: 12,
    expected_workspace_id: "workspace",
  });
  expect(f.options.onFinish).toHaveBeenCalledWith(["lead"]);
});

it("notifies when project commands remain unconfirmed", async () => {
  const f = fixture();
  f.post.mockImplementationOnce(async (_path, body) => ({
    id: body.id,
    request_id: body.request_id,
    outcome: "deleted",
    stopped: StopOutcome.unconfirmed,
    deletedAgentIds: ["lead"],
    deletedRoomIds: [],
  }));
  f.render().open("project");
  f.render().dialog.onConfirm();
  f.render();
  await f.settle();
  expect(f.options.onFinish).toHaveBeenCalledWith(["lead"]);
  expect(f.options.notify).toHaveBeenCalledWith(
    "The project was deleted, but stopping its commands could not be confirmed.",
  );
});

it("keeps unknown visible across SSE retirement and recovers only the saved request", async () => {
  const f = fixture();
  f.post.mockRejectedValueOnce(new TypeError("response lost"));
  f.render().open("project");
  f.render().dialog.onConfirm();
  f.render();
  await f.settle();
  expect(f.render().dialog.phase).toBe("unknown");
  expect(f.options.onFinish).not.toHaveBeenCalled();

  await f.settle();
  const request = required(f.post.mock.calls[0])[1];
  f.options.data = {
    ...data,
    threads: [],
    runtime: { ...data.runtime, projects: [] },
  };
  const retired = f.render();
  expect(f.options.onFinish).toHaveBeenCalledWith(["lead"]);
  expect(retired.dialog.phase).toBe("unknown");
  retired.dialog.onCancel();
  expect(f.render().dialog.phase).toBe("unknown");
  expect(f.post).toHaveBeenCalledOnce();
  retired.dialog.onRecover();
  retired.dialog.onRecover();
  f.render();
  await f.settle();
  expect(f.post).toHaveBeenCalledTimes(2);

  expect(required(f.post.mock.calls[1])[1]).toEqual(request);
  expect(f.render().dialog.name).toBeNull();
});

it("restores a saved intention after remount without automatically replaying HTTP", async () => {
  const f = fixture();
  f.post.mockRejectedValueOnce(new TypeError("response lost"));
  f.render().open("project");
  f.render().dialog.onConfirm();
  f.render();
  await f.settle();
  stops.splice(0).forEach((stop) => stop());
  const stop = f.render().start();
  stops.push(stop);
  f.render();
  const restored = await f.settle();
  expect(restored.dialog.phase).toBe("unknown");
  expect(f.post).toHaveBeenCalledOnce();
  restored.dialog.onRecover();
  f.render();
  await f.settle();

  expect(required(f.post.mock.calls[1])[1]).toEqual(
    required(f.post.mock.calls[0])[1],
  );
});

it("definite refusal restores readers and shows its reason without a refresh", async () => {
  const f = fixture();
  f.post.mockRejectedValueOnce(
    new ApiError(
      { code: "project_remote_scope", error: "Project spans servers" },
      409,
    ),
  );
  f.render().open("project");
  f.render().dialog.onConfirm();
  f.render();
  await f.settle();
  expect(f.render().dialog.phase).toBe("rejected");
  expect(f.render().dialog.error).toContain("Project spans servers");
  expect(f.options.onResume).toHaveBeenCalledWith(new Set(["lead"]));
  expect(f.options.onFinish).not.toHaveBeenCalled();
  expect(f.values.size).toBe(0);
});

it("never combines an old project sequence with another workspace's identity", () => {
  const f = fixture();
  f.options.workspaceId = "different";
  f.render().open("project");
  expect(f.render().dialog.name).toBeNull();
  f.options.workspaceId = "workspace";
  f.render().open("project");
  const confirming = f.render();
  f.options.workspaceId = "different";
  f.render();
  confirming.dialog.onConfirm();
  f.render();
  expect(f.post).not.toHaveBeenCalled();
  expect(f.values.size).toBe(0);
});

it("does not treat partial or foreign-workspace baselines as project retirement", () => {
  const f = fixture();
  f.render();
  f.options.data = {
    ...data,
    entitiesComplete: false,
    threads: [],
    runtime: { ...data.runtime, projects: [] },
  };
  f.render();
  expect(f.options.onFinish).not.toHaveBeenCalled();
  f.options.data = {
    ...f.options.data,
    entitiesComplete: true,
    entityWorkspaceId: "other",
  };
  f.render();
  expect(f.options.onFinish).not.toHaveBeenCalled();
});

it("remembers readers when chat and project tombstones arrive in separate streamed publications", () => {
  const f = fixture();
  f.render();
  f.options.data = { ...data, threads: [] };
  f.render();
  f.options.data = {
    ...f.options.data,
    runtime: { ...data.runtime, projects: [] },
  };
  f.render();
  expect(f.options.onFinish).toHaveBeenCalledWith(["lead"]);
});

it("never applies an old workspace's late deletion response to the current drafts", async () => {
  const f = fixture();
  let complete!: (value: unknown) => void;
  f.post.mockReturnValueOnce(
    new Promise((resolve) => {
      complete = resolve;
    }),
  );
  f.render().open("project");
  f.render().dialog.onConfirm();
  f.render();
  await f.settle();
  const request = required(f.post.mock.calls[0])[1];
  f.options.workspaceId = "new-workspace";
  f.options.data = { ...data, entityWorkspaceId: "new-workspace" };
  f.render();
  complete({
    id: request.id,
    request_id: request.request_id,
    outcome: "deleted",
    stopped: StopOutcome.confirmed,
    deletedAgentIds: ["lead"],
    deletedRoomIds: [],
  });
  await f.settle();
  expect(f.options.onFinish).not.toHaveBeenCalled();
  expect(f.render().dialog.name).toBeNull();
});

it("retains suppression after HTTP success until the listed entities retire through SSE", async () => {
  const f = fixture();
  f.render().open("project");
  f.render().dialog.onConfirm();
  f.render();
  await f.settle();
  expect(f.options.onFinish).toHaveBeenCalledWith(["lead"]);
  expect(f.options.onResume).not.toHaveBeenCalled();
  expect(f.options.onSuspend).toHaveBeenLastCalledWith(new Set(["lead"]));
  f.options.data = {
    ...data,
    threads: [],
    runtime: { ...data.runtime, projects: [] },
  };
  f.render();
  expect(f.options.onResume).toHaveBeenCalled();
});

it("registry-only project removal preserves surviving chats, rooms, drafts and selection", () => {
  const f = fixture();
  f.render();
  f.options.data = { ...data, runtime: { ...data.runtime, projects: [] } };
  f.render();
  expect(f.options.onFinish).not.toHaveBeenCalled();
  expect(f.options.onResume).not.toHaveBeenCalled();
});

it("not_applied multi-server refusal restores readers and releases the intention", async () => {
  const f = fixture();
  f.post.mockRejectedValueOnce(
    new ApiError(
      {
        code: "project_remote_scope",
        outcome: "not_applied",
        error: "Project spans servers",
      },
      409,
    ),
  );
  f.render().open("project");
  f.render().dialog.onConfirm();
  f.render();
  await f.settle();
  expect(f.render().dialog.phase).toBe("rejected");
  expect(f.render().dialog.error).toContain("Project spans servers");
  expect(f.options.onResume).toHaveBeenCalledWith(new Set(["lead"]));
  expect(f.values.size).toBe(0);
});

it("does not release confirmed IDs from an incomplete projection", async () => {
  const f = fixture();
  let complete!: (value: unknown) => void;
  f.post.mockReturnValueOnce(
    new Promise((resolve) => {
      complete = resolve;
    }),
  );
  f.render().open("project");
  f.render().dialog.onConfirm();
  f.render();
  await f.settle();
  const request = required(f.post.mock.calls[0])[1];
  f.options.data = { ...data, entitiesComplete: false, threads: [] };
  f.render();
  complete({
    id: request.id,
    request_id: request.request_id,
    outcome: "deleted",
    stopped: StopOutcome.confirmed,
    deletedAgentIds: ["lead"],
    deletedRoomIds: [],
  });
  await f.settle();
  expect(f.options.onResume).not.toHaveBeenCalled();
  expect(f.options.onSuspend).toHaveBeenLastCalledWith(new Set(["lead"]));
});

it("disposing the owner prevents a scheduled deletion from starting", async () => {
  const f = fixture();
  f.render().open("project");
  f.render().confirm();
  f.close();
  await f.settle();
  expect(f.post).not.toHaveBeenCalled();
});
