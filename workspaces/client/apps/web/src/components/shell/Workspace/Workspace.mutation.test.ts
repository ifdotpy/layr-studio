// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, createElement, type ComponentProps, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { Provider as AreaProvider } from "../../../app/ServerAreasProvider.tsx";
import {
  conversationActionsFixture,
  observableFixture,
} from "../integration-support/observableFixture.tsx";
import Workspace from "./Workspace.tsx";

function workspaceFixture(...args: Parameters<typeof observableFixture>) {
  const fixture = observableFixture(...args);
  const areas = conversationActionsFixture(undefined, fixture.store);
  areas.scope.services.api.get = api.get;
  areas.scope.services.api.post = api.post;
  return {
    ...fixture,
    Provider: ({ children }: { children?: ReactNode }) =>
      createElement(
        AreaProvider,
        { value: areas },
        createElement(fixture.Provider, {}, children),
      ),
    store: {
      ...fixture.store,
      dispose() {
        areas.dispose();
        areas.scope.services.close();
        fixture.store.dispose();
      },
    },
  };
}

const api = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  replaceStream: vi.fn(),
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => api,
    useServerResources: () => ({ replaceStream: api.replaceStream }),
  }),
);
vi.mock("../WorkspacePanelHost/WorkspacePanelHost.tsx", () => ({
  WorkspacePanelHost: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("@mantine/core", async (original) => {
  const actual = await original<typeof import("@mantine/core")>();
  const children = ({ children }: { children?: ReactNode }) => children;
  const button = ({
    children,
    onClick,
    disabled,
    type,
    "aria-label": label,
  }: ComponentProps<"button">) =>
    createElement(
      "button",
      { onClick, disabled, type, "aria-label": label },
      children,
    );
  return {
    ...actual,
    Button: Object.assign(button, actual.Button),
    UnstyledButton: Object.assign(
      (props: ComponentProps<"button">) => button(props),
      actual.UnstyledButton,
    ),
    Badge: Object.assign(children, actual.Badge),
    Modal: Object.assign(
      ({ opened, children }: { opened: boolean; children: ReactNode }) =>
        opened ? children : null,
      actual.Modal,
    ),
  };
});

it("deleting a profile reloads the local list without replacing the stream", async () => {
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
  const profile = {
    id: "profile",
    name: "Saved profile",
    role: "implementer",
    model: "gpt-6-astra",
    effort: "high",
    instructions: "Review work",
  };
  let profiles = [profile];
  api.get.mockImplementation(async (path) => {
    if (path !== "/api/profiles") throw new Error(`Unexpected read: ${path}`);
    return { profiles };
  });
  api.post.mockImplementation(async () => {
    profiles = [];
    return {};
  });
  const f = workspaceFixture([]);
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(
            f.Provider,
            null,
            createElement(Workspace, {
              opened: true,
              initialSection: "profiles",
              onClose() {},
              onSelect() {},
              notify() {},
            }),
          ),
        ),
      ),
    );
    expect(host.textContent).toContain("Saved profile");
    const remove = () =>
      [...host.querySelectorAll("button")].find(
        (button) => button.textContent === "Delete",
      )!;
    await act(async () => remove().click());
    await act(async () =>
      [...host.querySelectorAll("button")]
        .find((button) => button.textContent === "Delete profile")!
        .click(),
    );
    expect(api.post).toHaveBeenCalledWith("/api/profiles", {
      id: "profile",
      action: "delete",
      role: profile.role,
      instructions: profile.instructions,
    });
    expect(host.textContent).toContain("No saved worker profiles.");
    expect(host.textContent).not.toContain("Saved profile");
    expect(api.replaceStream).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    f.store.dispose();
    vi.unstubAllGlobals();
  }
});

import type { ApiReadMetadata, GetResult } from "../../../api.ts";
import {
  AgentSource,
  studio_api__agents__models__AgentRole,
} from "../../../generated/api.ts";
import type { Agent } from "../../../types.ts";

async function mountPanel(section: string, agent?: Agent) {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const fixture = workspaceFixture(agent ? [agent] : []);
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  await act(async () =>
    root.render(
      createElement(
        MantineProvider,
        { env: "test" },
        createElement(
          fixture.Provider,
          null,
          createElement(Workspace, {
            opened: true,
            initialSection: section,
            agent,
            onClose() {},
            onSelect() {},
            notify() {},
          }),
        ),
      ),
    ),
  );
  return {
    host,
    close: async () => {
      await act(async () => root.unmount());
      host.remove();
      fixture.store.dispose();
      vi.unstubAllGlobals();
    },
  };
}

it("keeps workspace pending feedback after leaving a saving section", async () => {
  const pending = Promise.withResolvers<{}>();
  api.get.mockReset().mockResolvedValue({ checkpoints: [], files: [] });
  api.post.mockReset().mockReturnValue(pending.promise);
  const panel = await mountPanel("checkpoints", {
    id: "agent",
    source: AgentSource.managed,
  });
  try {
    await act(async () =>
      panel.host
        .querySelector("form")
        ?.dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        ),
    );
    expect(api.post).toHaveBeenCalledWith("/api/checkpoint", {
      agent: "agent",
      label: "Manual checkpoint",
    });
    await act(async () =>
      [...panel.host.querySelectorAll("button")]
        .find((button) => button.textContent === "Plan")
        ?.click(),
    );
    const feedback = () =>
      panel.host.querySelector(
        '[aria-label="Workspace operation in progress"]',
      );
    expect(feedback()).not.toBeNull();
    await act(async () => pending.resolve({}));
    expect(feedback()).toBeNull();
  } finally {
    pending.resolve({});
    await panel.close();
  }
});

it("the capabilities panel renders a deferred tools response without another user action", async () => {
  const pending = Promise.withResolvers<GetResult<"/api/capabilities">>();
  api.get.mockReset().mockReturnValueOnce(pending.promise);
  const panel = await mountPanel("tools", {
    id: "agent",
    source: AgentSource.managed,
  });
  try {
    expect(panel.host.querySelector(".workspace-load")).not.toBeNull();
    await act(async () =>
      pending.resolve({
        agent: "agent",
        at: 1,
        model: "model",
        role: studio_api__agents__models__AgentRole.orchestrator,
        managed: [
          { name: "deferred-tool", description: "Deferred tool catalog" },
        ],
        observed: [],
        observedNative: [],
        nativeInventory: "Native tools",
        skills: [],
        servers: [],
        errors: [],
        mcp: [],
        mcpSession: null,
      }),
    );
    expect(panel.host.textContent).toContain("deferred-tool");
    expect(panel.host.querySelector(".workspace-load")).toBeNull();
    expect(api.get).toHaveBeenCalledTimes(1);
  } finally {
    await panel.close();
  }
});

it("the resource status shows a rejected panel read, recovery, refresh errors and 304 error clearing", async () => {
  const pending = Promise.withResolvers<GetResult<"/api/profiles">>();
  const empty: GetResult<"/api/profiles"> = { profiles: [] };
  api.get
    .mockReset()
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValueOnce(empty)
    .mockRejectedValueOnce(new Error("Refresh failed"))
    .mockImplementationOnce(
      async (_path, options: { readMetadata: ApiReadMetadata }) => {
        options.readMetadata.notModified = true;
        return undefined;
      },
    );
  const panel = await mountPanel("profiles");
  const refresh = async () => {
    const button = panel.host.querySelector<HTMLButtonElement>(
      '[aria-label="Refresh workspace"]',
    );
    expect(button).not.toBeNull();
    await act(async () => button?.click());
  };
  try {
    expect(panel.host.querySelector(".workspace-load")).not.toBeNull();
    await act(async () => pending.reject(new Error("Initial read failed")));
    expect(panel.host.querySelector('[role="alert"]')?.textContent).toBe(
      "Initial read failed",
    );
    expect(panel.host.querySelector(".workspace-load")).toBeNull();
    await refresh();
    expect(panel.host.textContent).toContain("No saved worker profiles.");
    expect(panel.host.querySelector('[role="alert"]')).toBeNull();
    await refresh();
    expect(panel.host.querySelector('[role="alert"]')?.textContent).toBe(
      "Refresh failed",
    );
    await refresh();
    expect(panel.host.querySelector('[role="alert"]')).toBeNull();
    expect(panel.host.textContent).toContain("No saved worker profiles.");
    expect(api.get).toHaveBeenCalledTimes(4);
  } finally {
    await panel.close();
  }
});

it("discards a transient comment draft when leaving its section", async () => {
  const changes: GetResult<"/api/changes"> = {
    scope: "chat",
    git: true,
    patch: "",
    files: [],
    diff: "diff --git a/file.ts b/file.ts\n+++ b/file.ts\n@@ -0,0 +1 @@\n+line",
  };
  api.get.mockReset().mockResolvedValue(changes);
  const panel = await mountPanel("changes", {
    id: "agent",
    source: AgentSource.managed,
  });
  try {
    const comment = panel.host.querySelector<HTMLButtonElement>(
      '[aria-label="Comment on file.ts line 1"]',
    );
    expect(comment).not.toBeNull();
    await act(async () => comment?.click());
    expect(panel.host.querySelector("textarea")).not.toBeNull();
    const section = (name: string) =>
      [...panel.host.querySelectorAll("button")].find(
        (button) => button.textContent === name,
      )!;
    await act(async () => section("Plan").click());
    await act(async () => section("Changes").click());
    expect(panel.host.querySelector("textarea")).toBeNull();
  } finally {
    await panel.close();
  }
});

it("a lead-keyed Workspace remount starts on that lead's entry section", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const first: Agent = {
    id: "first",
    isLead: true,
    source: AgentSource.managed,
  };
  const second: Agent = {
    id: "second",
    isLead: true,
    source: AgentSource.managed,
  };
  api.get.mockReset().mockImplementation(async (path) =>
    path === "/api/changes"
      ? { scope: "chat", git: true, patch: "", files: [], diff: "" }
      : {
          managed: [],
          observedNative: [],
          skills: [],
          errors: [],
          mcp: [],
          servers: [],
        },
  );
  const fixture = workspaceFixture([first, second]);
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const render = (agent: Agent) =>
    root.render(
      createElement(
        MantineProvider,
        { env: "test" },
        createElement(
          fixture.Provider,
          null,
          createElement(Workspace, {
            key: agent.id,
            opened: true,
            initialSection: "tools",
            agent,
            onClose() {},
            onSelect() {},
            notify() {},
          }),
        ),
      ),
    );
  try {
    await act(async () => render(first));
    const changes = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Changes",
    );
    await act(async () => changes?.click());
    expect(host.querySelector('[aria-label="Open a file"]')).not.toBeNull();
    await act(async () => render(second));
    expect(host.querySelector('[aria-label="Filter tools"]')).not.toBeNull();
    expect(host.querySelector('[aria-label="Open a file"]')).toBeNull();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    fixture.store.dispose();
    vi.unstubAllGlobals();
  }
});

it("renders one selected-agent rule list and resumes an enabled-state pause", async () => {
  const rules: GetResult<"/api/rules"> = {
    rules: [
      {
        id: "paused",
        agent: "agent",
        name: "Paused rule",
        kind: "interval",
        status: "active",
        enabled: false,
      },
      {
        id: "active",
        agent: "agent",
        name: "Active rule",
        kind: "interval",
        enabled: true,
      },
      {
        id: "foreign",
        agent: "other",
        name: "Other agent rule",
        kind: "interval",
      },
    ],
  };
  api.get.mockReset().mockResolvedValue(rules);
  api.post.mockReset().mockResolvedValue({});
  const panel = await mountPanel("rules", {
    id: "agent",
    source: AgentSource.managed,
  });
  try {
    expect(panel.host.querySelectorAll(".workspace-row")).toHaveLength(2);
    expect(panel.host.textContent).not.toContain("Other agent rule");
    const resume = [...panel.host.querySelectorAll("button")].find(
      (button) => button.textContent === "Resume",
    );
    expect(resume).toBeDefined();
    await act(async () => resume?.click());
    expect(api.post).toHaveBeenCalledWith("/api/rules", {
      agent: "agent",
      id: "paused",
      action: "resume",
    });
    expect(panel.host.textContent).not.toContain("No rules for this agent.");
  } finally {
    await panel.close();
  }
});
