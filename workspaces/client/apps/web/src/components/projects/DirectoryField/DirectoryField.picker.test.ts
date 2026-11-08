import type { ReactElement, ReactNode } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { required } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";

const h = vi.hoisted(() => ({
  cursor: 0,
  cells: [] as unknown[],
  native: false,
  pick: vi.fn(),
  change: vi.fn(),
}));
vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useState: (initial: unknown) => {
    const index = h.cursor++;
    if (!(index in h.cells))
      h.cells[index] = typeof initial === "function" ? initial() : initial;
    return [
      h.cells[index],
      (value: unknown) => {
        h.cells[index] = value;
      },
    ];
  },
}));
const owner = {};
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerApi: () => ({}),
    useServerStore: () => owner,
    useServerFileDialogs: () => ({
      canPickDirectory: () => h.native,
      pickDirectory: h.pick,
    }),
  }),
);
vi.mock("../ProjectDirectoryPicker/ProjectDirectoryPicker.tsx", () => ({
  default: "directory-picker",
  browseDirectory: () => ({}),
}));
import DirectoryField from "./DirectoryField.tsx";
type Props = {
  children?: ReactNode;
  rightSection?: ReactNode;
  onClick?: () => void;
  onSelect?: (path: string) => Promise<void>;
  "aria-label"?: string;
};
function find(
  node: ReactNode,
  predicate: (element: ReactElement<Props>) => boolean,
): ReactElement<Props> | undefined {
  if (Array.isArray(node))
    return node.map((child) => find(child, predicate)).find(Boolean);
  if (!node || typeof node !== "object" || !("props" in node)) return undefined;
  const element = node as ReactElement<Props>;
  return predicate(element)
    ? element
    : (find(element.props.rightSection, predicate) ??
        find(element.props.children, predicate));
}
function render() {
  h.cursor = 0;
  return DirectoryField({ label: "Folder", value: "/old", onChange: h.change });
}
function choose() {
  required(
    required(
      find(
        render(),
        (element) => element.props["aria-label"] === "Choose folder",
      ),
    ).props.onClick,
  )();
}
beforeEach(() => {
  h.cursor = 0;
  h.cells = [];
  h.native = false;
  h.pick.mockReset();
  h.change.mockReset();
});
it("uses the native dialog when available", async () => {
  h.native = true;
  h.pick.mockResolvedValue("/native");
  choose();
  await Promise.resolve();
  await Promise.resolve();
  expect(h.change).toHaveBeenCalledWith("/native");
});
it("uses the server picker otherwise", async () => {
  choose();
  const picker = required(
    find(render(), (element) => element.type === "directory-picker"),
  );
  await required(picker.props.onSelect)("/server");
  expect(h.change).toHaveBeenCalledWith("/server");
  expect(h.pick).not.toHaveBeenCalled();
});
