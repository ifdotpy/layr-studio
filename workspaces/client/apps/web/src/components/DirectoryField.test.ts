import { MantineProvider } from "@mantine/core";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it, vi } from "vitest";

vi.mock("../servers/ServerServicesContext", () => ({
  useServerFileDialogs: () => ({
    canPickDirectory: () => false,
    pickDirectory: vi.fn(),
  }),
}));
vi.mock("./ProjectDirectoryPicker", () => ({ default: () => null }));
import DirectoryField from "./DirectoryField";

const render = (readOnly?: boolean) =>
  renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(DirectoryField, {
        label: "Default store directory",
        value: "/store",
        onChange: () => {},
        ...(readOnly !== undefined ? { readOnly } : {}),
      }),
    ),
  );

it("offers the folder picker next to the typed path", () => {
  const html = render();
  expect(html).toContain('value="/store"');
  expect(html).toContain('aria-label="Choose default store directory"');
});

it("keeps a read-only path without the picker", () => {
  expect(render(true)).not.toContain("Choose default store directory");
});

it("walks up from a missing saved path by its text", async () => {
  const { parentPath } = await vi.importActual<
    typeof import("./ProjectDirectoryPicker")
  >("./ProjectDirectoryPicker");
  expect(parentPath("/a/b/store/")).toBe("/a/b");
  expect(parentPath("/a")).toBe("/");
  expect(parentPath("/")).toBe("/");
});
