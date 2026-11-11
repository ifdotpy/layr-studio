// @vitest-environment happy-dom
import { act, createElement, type ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { beforeEach, afterEach, expect, it, vi } from "vitest";
import { ProjectSave } from "../../../app/projects/ProjectSave.ts";
import { localSidebarBackend } from "../../sidebar/support/services.tsx";
import { createStateMirror } from "../../../generated/observableResources.ts";
import { ProjectNameForm } from "./ProjectNameForm.tsx";
import { MoveChatForm } from "../MoveChatForm/MoveChatForm.tsx";
import ProjectAccount from "../ProjectAccount/ProjectAccount.tsx";
let host: HTMLDivElement, root: ReturnType<typeof createRoot>;
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  host = document.body.appendChild(document.createElement("div"));
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.unstubAllGlobals();
});
const render = async (child: ReactElement) =>
  act(async () => root.render(createElement(MantineProvider, null, child)));
const submit = async () => {
  const form = host.querySelector("form");
  if (!form) throw Error("Missing form");
  await act(async () =>
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    ),
  );
};
function saving() {
  const post = vi.fn<(path: string, body: unknown, options: unknown) => void>();
  const backend = {
    ...localSidebarBackend,
    post: async (...args: Parameters<typeof localSidebarBackend.post>) => {
      post(...args);
      throw new Error("Fixture stopped");
    },
  };
  return {
    post,
    save: new ProjectSave(backend, "/api/projects", async () => {}),
    move: new ProjectSave(backend, "/api/organization", async () => {}),
  };
}
it("renames the replacement project using its current save owner", async () => {
  const first = saving(),
    second = saving();
  await render(
    createElement(ProjectNameForm, {
      project: { id: "a", path: "/a", name: "A" },
      save: first.save,
    }),
  );
  await render(
    createElement(ProjectNameForm, {
      project: { id: "b", path: "/b", name: "B" },
      save: second.save,
    }),
  );
  await submit();
  expect(first.post).not.toHaveBeenCalled();
  expect(second.post).toHaveBeenCalledWith(
    "/api/projects",
    expect.objectContaining({ path: "/b", name: "B" }),
    expect.anything(),
  );
});
it("moves the replacement chat using its own revision snapshot", async () => {
  const first = saving(),
    second = saving();
  await render(
    createElement(MoveChatForm, {
      project: { id: "a", path: "/a" },
      agent: { id: "a", projectFolder: "old", projectFolderRevision: 1 },
      save: first.move,
    }),
  );
  await render(
    createElement(MoveChatForm, {
      project: { id: "b", path: "/b" },
      agent: { id: "b", projectFolder: null, projectFolderRevision: 4 },
      save: second.move,
    }),
  );
  await submit();
  expect(first.post).not.toHaveBeenCalled();
  expect(second.post).toHaveBeenCalledWith(
    "/api/organization",
    expect.objectContaining({
      id: "b",
      project_path: "/b",
      expected_revision: 4,
    }),
    expect.anything(),
  );
});
it("initializes the later-loaded project's settings and reads replacement baselines", async () => {
  const mirror = createStateMirror();
  mirror.apply([
    {
      collection: "project",
      id: "project",
      value: { id: "project", path: "/project", workerBaseRef: "main" },
    },
  ]);
  const loaded = mirror.current.projects.get("project");
  if (!loaded) throw Error("Missing project");
  const { save } = saving();
  const props = {
    path: "/project",
    accounts: {
      accounts: [],
      archivedAccounts: [],
      defaultAccountGroups: [],
      defaultAccountKey: "default",
      supportsDelete: true,
      supportsDisconnect: true,
    },
    defaultAccountKey: "default",
    saves: {
      account: save,
      worker: save,
      order: save,
      defaultOrder: new ProjectSave(
        localSidebarBackend,
        "/api/accounts/default-groups",
        async () => {},
      ),
    },
  };
  await render(createElement(ProjectAccount, props));
  await render(createElement(ProjectAccount, { ...props, project: loaded }));
  expect(
    host.querySelector<HTMLInputElement>("#project-worker-base")?.value,
  ).toBe("main");
  const replacement = createStateMirror();
  replacement.apply([
    {
      collection: "project",
      id: "project",
      value: { id: "project", path: "/project", workerBaseRef: "release" },
    },
  ]);
  await render(
    createElement(ProjectAccount, {
      ...props,
      project: replacement.current.projects.get("project"),
    }),
  );
  expect(
    host.querySelector<HTMLInputElement>("#project-worker-base")?.value,
  ).toBe("main");
  expect(host.querySelector('[aria-label="Save worker base"]')).not.toBeNull();
});
