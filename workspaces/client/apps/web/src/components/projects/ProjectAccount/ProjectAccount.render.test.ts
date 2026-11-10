// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { createStateMirror } from "../../../generated/observableResources.ts";
import ProjectAccount from "./ProjectAccount.tsx";

vi.mock("../support/useProjectSave.ts", () => ({
  useProjectSave: () => ({
    pending: false,
    frozen: false,
    error: "",
    submit: vi.fn(),
  }),
}));

it("updates project change indicators from the mounted model without a parent render", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
  }));
  const mirror = createStateMirror();
  const publish = (workerBaseRef: string, accountGroups: string[]) =>
    mirror.apply([
      {
        collection: "project",
        id: "project",
        value: {
          id: "project",
          path: "/project",
          workerBaseRef,
          accountGroups,
        },
      },
    ]);
  publish("main", ["personal"]);
  const project = mirror.current.projects.get("project");
  if (!project) throw new Error("Missing project fixture");
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ProjectAccount, {
            path: "/project",
            project,
            accounts: {
              accounts: [],
              archivedAccounts: [],
              defaultAccountKey: "default",
              defaultAccountGroups: [],
              supportsDelete: true,
              supportsDisconnect: true,
            },
            defaultAccountKey: "default",
            saved: async () => {},
          }),
        ),
      ),
    );
    const workerSave = () =>
      host.querySelector('[aria-label="Save worker base"]');
    const orderSave = () =>
      host.querySelector('[aria-label="Save account order"]');
    expect(workerSave()).toBeNull();
    expect(orderSave()).toBeNull();
    await act(async () => publish("release", ["work"]));
    expect(mirror.current.projects.get("project")).toBe(project);
    expect(workerSave()).not.toBeNull();
    expect(orderSave()).not.toBeNull();
    // The local edits remain intact while their saved baseline changes.
    expect(
      host.querySelector<HTMLInputElement>("#project-worker-base")?.value,
    ).toBe("main");
    await act(async () => publish("main", ["personal"]));
    expect(workerSave()).toBeNull();
    expect(orderSave()).toBeNull();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});
