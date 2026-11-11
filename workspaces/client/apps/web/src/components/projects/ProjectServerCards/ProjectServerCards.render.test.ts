import { ProjectServerSelection } from "../../../app/projects/CreationOwner.ts";
import type { NewChatModelStore } from "../../settings/NewChatModelSettings/NewChatModelStore.ts";
import { createServerServices } from "../../../servers/serverServices.ts";
import { localServer } from "../../../servers/registry.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import type { ProjectServerChoice } from "../../../servers/projectLocations.ts";
import ProjectServerCards from "./ProjectServerCards.tsx";
import { ChatWorkspaceMode } from "../../../generated/api.ts";

vi.mock("../../agents/support/WorkerModelPicker.tsx", async (load) => ({
  ...(await load<
    typeof import("../../agents/support/WorkerModelPicker.tsx")
  >()),
  createWorkerCatalog: () => ({
    active: true,
    pending: false,
    dispose: vi.fn(),
    retryError: "",
    models: [],
    loading: false,
    error: "",
    retry: vi.fn(),
  }),
}));
vi.mock(
  "../../settings/NewChatModelSettings/NewChatModelSettings.tsx",
  async (load) => ({
    ...(await load<
      typeof import("../../settings/NewChatModelSettings/NewChatModelSettings.tsx")
    >()),
    default: ({ model }: { model: NewChatModelStore }) =>
      createElement(
        "button",
        { onClick: () => model.patch({ model: "model" }) },
        "Choose model",
      ),
  }),
);

it("requires the newly selected server's model settings before starting", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const button = (label: string) => {
    const value = [...host.querySelectorAll("button")].find((node) =>
      node.textContent?.includes(label),
    );
    if (!value) throw new Error(`Missing ${label}`);
    return value;
  };
  const project = {
    id: "project",
    name: "Project",
    path: "/project",
    locations: [
      {
        serverId: "first",
        projectId: "first-project",
        path: "/first",
      },
      {
        serverId: "second",
        projectId: "second-project",
        path: "/second",
      },
    ],
  };
  const servers = ["first", "second"].map(
    (id) =>
      ({
        id,
        label: id,
        system: "server",
        nativeLayr: { status: "notInstalled" },
        defaultWorkspaceMode: ChatWorkspaceMode.worktree,
      }) satisfies ProjectServerChoice,
  );
  const service = createServerServices(localServer());
  const fixture = observableFixture();
  Object.defineProperty(service, "store", { value: fixture.store });
  fixture.apply([
    {
      collection: "accounts",
      id: "current",
      value: {
        accounts: [],
        archivedAccounts: [],
        defaultAccountGroups: [],
        defaultAccountKey: "default",
        supportsDelete: true,
        supportsDisconnect: true,
      },
    },
  ]);
  const owner = new ProjectServerSelection(
    project,
    () => servers,
    undefined,
    () => service,
  );
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ProjectServerCards, {
            project,
            servers,
            owner,
            onChoose: vi.fn(),
            onAdd: vi.fn(),
            onCancel: vi.fn(),
          }),
        ),
      ),
    );
    await act(async () => button("Choose model").click());
    expect(button("Start chat").disabled).toBe(false);
    await act(async () => button("second").click());
    expect(button("Start chat").disabled).toBe(true);
    await act(async () => button("Choose model").click());
    expect(button("Start chat").disabled).toBe(false);
  } finally {
    owner.dispose();
    service.close();
    fixture.store.dispose();
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});
