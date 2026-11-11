import type { ProjectServerChoice } from "../../../servers/projectLocations.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import type { DirectoryBrowse } from "./ProjectDirectoryPicker.tsx";
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerNet: () => ({}),
    useServerStore: () => ({}),
    useServerFileDialogs: () => ({ canPickDirectory: () => false }),
  }),
);
import ProjectDirectoryPicker from "./ProjectDirectoryPicker.tsx";

it("uses current choices and callbacks and blocks a removed server without replacing the draft", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const data = { path: "/folder", parent: "/", directories: [] };
  const initialBrowse: DirectoryBrowse = {
    path: data.path,
    directory: { current: { status: "live", data }, read: async () => data },
    matches: null,
  };
  const host = document.body.appendChild(document.createElement("div")),
    root = createRoot(host),
    first = vi.fn().mockResolvedValue(undefined),
    second = vi.fn().mockResolvedValue(undefined);
  const render = async (
    disabled: boolean,
    onSelect: typeof first,
    serverChoices: ProjectServerChoice[] = [
      { id: "local", label: "Server", disabled },
    ],
  ) =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ProjectDirectoryPicker, {
            initialBrowse,
            onSelect,
            serverChoices,
            submitLabel: "Choose folder",
          }),
        ),
      ),
    );
  const button = () => {
    const value = [...host.querySelectorAll("button")].find(
      (node) => node.textContent === "Choose folder",
    );
    if (!value) throw Error("Missing selection");
    return value;
  };
  try {
    await render(false, first);
    expect(button().disabled).toBe(false);
    await render(true, second);
    expect(button().disabled).toBe(true);
    await render(false, second);
    await act(async () => button().click());
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith("/folder", "local");
    second.mockClear();
    await render(false, second, [{ id: "other", label: "Other server" }]);
    expect(button().disabled).toBe(true);
    await act(async () => button().click());
    expect(second).not.toHaveBeenCalled();
    await render(false, second, []);
    expect(button().disabled).toBe(true);
    await act(async () => button().click());
    expect(second).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});
