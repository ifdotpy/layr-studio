// @vitest-environment happy-dom
import { act, createElement, type ReactNode, type ComponentProps } from "react";
import { MantineProvider } from "@mantine/core";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import Workspace from "./Workspace.tsx";
import { observableFixture } from "../integration-support/observableFixture.tsx";

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
  const f = observableFixture([]);
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

import {
  AgentSource,
  studio_api__agents__models__AgentRole,
} from "../../../generated/api.ts";
import type { Agent } from "../../../types.ts";
import type { ApiReadMetadata, GetResult } from "../../../api.ts";

async function mountPanel(section: string, agent?: Agent) {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const fixture = observableFixture(agent ? [agent] : []);
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
