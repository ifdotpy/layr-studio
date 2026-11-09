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
  }: ComponentProps<"button">) =>
    createElement("button", { onClick, disabled, type }, children);
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
