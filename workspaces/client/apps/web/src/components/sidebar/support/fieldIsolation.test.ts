import { createSidebarSourceStore } from "../../../servers/sidebarSourceFixture.ts";
import { stateFromFixture } from "../../../servers/sidebarSourceFixture.ts";
import { observable, runInAction } from "mobx";
import NativeSidebar from "../Sidebar/Sidebar.tsx";
import { SidebarListOwner } from "../../../app/shell/SidebarOwner.ts";
import { useSidebarState as awaitState } from "./SidebarState.tsx";
// @vitest-environment happy-dom
import { act, createElement, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import Sidebar, { sidebarFixtureAreas } from "../Sidebar/Sidebar.fixture.tsx";
import { SidebarCommandsProvider } from "./commands.ts";
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import {
  createStateMirror,
  serializeStateCollections,
} from "../../../generated/observableResources.ts";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import { componentSnapshot as snapshotFixture } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";

const frame = vi.hoisted(() => ({
  compact: undefined as ((value: boolean) => void) | undefined,
}));
vi.mock("../SidebarFrame/SidebarFrame.tsx", async (original) => ({
  ...(await original<typeof import("../SidebarFrame/SidebarFrame.tsx")>()),
  SidebarFrame: ({ children }: { children: ReactNode }) => {
    frame.compact = awaitState().visibility.recordCompact;
    return createElement("aside", null, children);
  },
  restoreSidebarFocus() {},
}));
it("folder field edits render only the folder branch and preserve unrelated headings and rows", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal("location", new URL("http://studio.test/"));
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent() {
      return true;
    },
  }));
  const mirror = createStateMirror();
  const apply = (folder: string, name = "Project") =>
    mirror.apply([
      {
        collection: "agentSummary",
        id: "chat",
        value: agentSummaryFixture("chat", { name: "Chat", projectPath: "/p" }),
      },
      {
        collection: "project",
        id: "p",
        value: {
          id: "p",
          path: "/p",
          name,
          folders: [{ id: "f", name: folder, parentId: null }],
        },
      },
      {
        collection: "project",
        id: "q",
        value: { id: "q", path: "/q", name: "Unrelated", folders: [] },
      },
    ]);
  apply("Before");
  const renders: string[] = [];
  window.__studioPromptComposerRenderProbe = (name) => renders.push(name);
  window.__studioSidebarRenderProbe = (name) => renders.push(name);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const fixture = observableFixture();
  const close = vi.fn();
  try {
    await act(async () =>
      root.render(
        createElement(
          ServerStoreProvider,
          { value: fixture.store },
          createElement(
            MantineProvider,
            null,
            createElement(
              SidebarCommandsProvider,
              {
                value: {
                  onSearch() {},
                  open() {},
                  newChat() {},
                  addProject() {},
                  changeProject() {},
                  projectAccount() {},
                  async rename() {},
                  remove() {},
                  markUnread() {},
                },
              },
              createElement(Sidebar, {
                data: Object.assign(snapshotFixture([]), {
                  token: "",
                  stateDir: "/fixture",
                }),
                collections: mirror.current,
                opened: null,
                creating: false,
                mobile: false,
                close,
                markingRead: new Set<string>(),
              }),
            ),
          ),
        ),
      ),
    );
    renders.length = 0;
    const unchanged = serializeStateCollections(mirror.current);
    for (const name of ["Created", "Updated", "Updated again"]) {
      await act(async () =>
        mirror.apply([
          ...unchanged,
          {
            collection: "rule",
            id: "unrelated",
            value: { id: "unrelated", name },
          },
        ]),
      );
      expect(renders).toEqual([]);
    }
    await act(async () => apply("After"));
    expect(host.textContent).toContain("After");
    expect(renders).not.toContain("sidebar");
    expect(renders).not.toContain("ProjectHeading");
    expect(renders).not.toContain("SidebarProjectList");
    expect(renders).toContain("FolderBranch");
    renders.length = 0;
    await act(async () => apply("After", "Renamed project"));
    expect(host.textContent).toContain("Renamed project");
    expect(renders).toContain("ProjectHeading");
    expect(renders).not.toContain("sidebar");
    expect(renders).not.toContain("SidebarProjectList");
    expect(renders).not.toContain("SidebarProjectSection");
    expect(renders).not.toContain("FolderBranch");
    await act(async () => frame.compact?.(true));
    await act(async () =>
      host.querySelector<HTMLButtonElement>('[data-chat="chat"]')!.click(),
    );
    expect(close).toHaveBeenCalledOnce();
  } finally {
    await act(async () => root.unmount());
    fixture.store.dispose();
    host.remove();
    delete window.__studioPromptComposerRenderProbe;
    delete window.__studioSidebarRenderProbe;
    vi.unstubAllGlobals();
  }
});

it("resolves a newly arrived combined row against the current services before layout sync", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal("location", new URL("http://studio.test/"));
  const { createMergedSidebarSelector } =
    await import("../../../servers/mergedSidebar.ts");
  const { createMergedSidebarServices } =
    await import("../../../servers/sidebarServices.ts");
  const { localSidebarBackend } = await import("./services.tsx");
  const { sidebarSnapshot } =
    await import("../../../servers/sidebarSnapshot.ts");
  const source = createSidebarSourceStore(),
    select = createMergedSidebarSelector();
  const fixture = observableFixture();
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  const model = select(source.sources(["local"], new Set(["local"])));
  const backends = new Map([["local", localSidebarBackend]]);
  const inputs = observable.box(
    {
      model,
      services: createMergedSidebarServices(model, backends, () => true, {}),
    },
    { deep: false },
  );
  const areas = sidebarFixtureAreas(fixture.store);
  const visibility = areas.sidebar;
  const boundary = {
    get services() {
      return inputs.get().services;
    },
    visibility,
    commands: {
      rename: areas.rename.rename,
      notify: areas.notices.notify,
    },
    selection: areas.navigation,
    preferences: areas.preferences,
    read: areas.read,
    creation: areas.creation,
    run: areas.run,
    get model() {
      return inputs.get().model;
    },
  };
  const owner = new SidebarListOwner(areas, boundary);
  const stop = owner.start();
  const render = () => {
    runInAction(() => {
      const model = select(source.sources(["local"], new Set(["local"])));
      inputs.set({
        model,
        services: createMergedSidebarServices(model, backends, () => true, {}),
      });
    });
    root.render(
      createElement(
        ServerStoreProvider,
        { value: fixture.store },
        createElement(
          MantineProvider,
          null,
          createElement(
            SidebarCommandsProvider,
            {
              value: {
                onSearch() {},
                open() {},
                newChat() {},
                addProject() {},
                changeProject() {},
                projectAccount() {},
                async rename() {},
                remove() {},
                markUnread() {},
              },
            },
            createElement(NativeSidebar, { owner }),
          ),
        ),
      ),
    );
  };
  try {
    await act(async () => render());
    source.update(
      "local",
      sidebarSnapshot(
        stateFromFixture(
          Object.assign(snapshotFixture([]), {
            token: "",
            stateDir: "/fixture",
          }),
        ),
        [
          {
            collection: "agentSummary",
            id: "arrived",
            value: agentSummaryFixture("arrived", { name: "Arrived chat" }),
          },
        ],
      ),
    );
    await act(async () => render());
    expect(host.textContent).toContain("Arrived chat");
  } finally {
    await act(async () => root.unmount());
    stop();
    areas.dispose();
    fixture.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});

it.each([true, false])(
  "keeps real row drag presentation and successive reorder dispatch current (indicator=%s)",
  async (checkIndicator) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    vi.stubGlobal("location", new URL("http://studio.test/"));
    const { localSidebarServices } = await import("./services.tsx");
    const { createSidebarViewModel } = await import("./viewModel.ts");
    const mirror = createStateMirror();
    mirror.apply(
      ["a", "b", "c"].map((id) => ({
        collection: "agentSummary" as const,
        id,
        value: agentSummaryFixture(id, { projectPath: "/p", name: id }),
      })),
    );
    const model = createSidebarViewModel(
      () => mirror.current,
      () => ({ query: "", archived: false, opened: null, rank: () => 0 }),
    );
    const rows = model.rows;
    const group = JSON.stringify(["items", "/p", null]);
    const data = Object.assign(
      snapshotFixture([], {
        projects: [{ id: "p", path: "/p", name: "Project", folders: [] }],
        sidebarOrder: { revision: 1, groups: { [group]: ["a", "b", "c"] } },
      }),
      { token: "", stateDir: "/fixture" },
    );
    data.stateDir = `drag-${checkIndicator}`;
    const submitted: unknown[] = [];
    let revision = 1;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const text =
          typeof init?.body === "string"
            ? init.body
            : input instanceof Request
              ? await input.text()
              : "{}";
        const body = JSON.parse(text);
        submitted.push(body.groups);
        return new Response(
          JSON.stringify({ revision: ++revision, groups: body.groups }),
          { headers: { "content-type": "application/json" } },
        );
      }),
    );
    const fixture = observableFixture(),
      host = document.createElement("div"),
      root = createRoot(host);
    fixture.apply([
      ...data.runtime.projects.map((value) => ({
        collection: "project" as const,
        id: value.id,
        value,
      })),
      {
        collection: "workspace",
        id: "fixture",
        value: {
          stateDir: data.stateDir,
          sidebarOrder: data.runtime.sidebarOrder,
        },
      },
    ]);
    document.body.append(host);
    try {
      await act(async () =>
        root.render(
          createElement(
            ServerStoreProvider,
            { value: fixture.store },
            createElement(
              MantineProvider,
              null,
              createElement(
                SidebarCommandsProvider,
                {
                  value: {
                    onSearch() {},
                    open() {},
                    newChat() {},
                    addProject() {},
                    changeProject() {},
                    projectAccount() {},
                    async rename() {},
                    remove() {},
                    markUnread() {},
                  },
                },
                createElement(Sidebar, {
                  data,
                  rows,
                  hideOldChatsThreshold: 100,
                  services: { ...localSidebarServices, available: () => true },
                  opened: null,
                  creating: false,
                  mobile: false,
                  close() {},
                  markingRead: new Set<string>(),
                }),
              ),
            ),
          ),
        ),
      );
      const button = (id: string) =>
        host.querySelector<HTMLButtonElement>(`[data-chat="${id}"]`)!;
      const originalButtons = ["a", "b", "c"].map(button);
      const transfer = new DataTransfer();
      const event = (kind: string) => {
        const value = new Event(kind, { bubbles: true, cancelable: true });
        Object.defineProperties(value, {
          dataTransfer: { value: transfer },
          clientY: { value: 90 },
        });
        return value;
      };
      const drag = async (from: string, to: string) => {
        vi.spyOn(button(to), "getBoundingClientRect").mockReturnValue(
          new DOMRect(0, 0, 100, 100),
        );
        await act(async () => {
          button(from).dispatchEvent(event("dragstart"));
        });
        await act(async () => {
          button(to).dispatchEvent(event("dragover"));
        });
        if (checkIndicator) expect(button(to).dataset.dropEdge).toBe("after");
        await act(async () => {
          button(to).dispatchEvent(event("drop"));
        });
      };
      await drag("a", "b");
      expect(submitted).toEqual([{ [group]: ["b", "a", "c"] }]);
      await drag("c", "b");
      expect(submitted).toEqual([
        { [group]: ["b", "a", "c"] },
        { [group]: ["b", "c", "a"] },
      ]);
      expect(button("b").dataset.dropEdge).toBeUndefined();
      ["a", "b", "c"].forEach((id, index) =>
        expect(button(id)).toBe(originalButtons[index]),
      );
    } finally {
      await act(async () => root.unmount());
      fixture.store.dispose();
      host.remove();
      vi.unstubAllGlobals();
    }
  },
);
