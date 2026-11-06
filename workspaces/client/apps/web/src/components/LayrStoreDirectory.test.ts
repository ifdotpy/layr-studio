import type { ReactElement, ReactNode } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { required } from "./componentTestFixtures";

const h = vi.hoisted(() => ({
  cursor: 0,
  cells: [] as unknown[],
  save: vi.fn(),
}));
vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useEffect: () => {},
  useState: (initial: unknown) => {
    const index = h.cursor++;
    if (!(index in h.cells)) h.cells[index] = initial;
    return [
      h.cells[index],
      (value: unknown) => {
        h.cells[index] = value;
      },
    ];
  },
}));
import DirectoryField from "./DirectoryField";
import { LayrStoreDirectory } from "./LayrStoreDirectory";
type Props = {
  children?: ReactNode;
  onClick?: () => void;
  onChange?: (value: string) => void;
  role?: string;
};
function find(
  node: ReactNode,
  predicate: (element: ReactElement<Props>) => boolean,
): ReactElement<Props> | undefined {
  if (Array.isArray(node))
    return node.map((child) => find(child, predicate)).find(Boolean);
  if (!node || typeof node !== "object" || !("props" in node)) return undefined;
  const element = node as ReactElement<Props>;
  return predicate(element) ? element : find(element.props.children, predicate);
}
function render() {
  h.cursor = 0;
  return LayrStoreDirectory({
    label: "Store",
    path: "/current",
    source: "project",
    overridden: true,
    save: h.save,
  });
}
function click(label: string) {
  required(
    required(find(render(), (element) => element.props.children === label))
      .props.onClick,
  )();
}
async function settle() {
  for (let i = 0; i < 8; i++) await Promise.resolve();
}
beforeEach(() => {
  h.cursor = 0;
  h.cells = [];
  h.save.mockReset();
  h.save.mockResolvedValue(undefined);
});
it("saves the chosen or typed directory", async () => {
  required(
    required(find(render(), (element) => element.type === DirectoryField)).props
      .onChange,
  )("/new");
  click("Save");
  await settle();
  expect(h.save).toHaveBeenCalledWith("/new");
});
it("persists null on reset", async () => {
  click("Reset");
  await settle();
  expect(h.save).toHaveBeenCalledWith(null);
});
it("shows save failures", async () => {
  h.save.mockRejectedValue(new Error("Store directory changed"));
  click("Reset");
  await settle();
  expect(
    required(find(render(), (element) => element.props.role === "alert")).props
      .children,
  ).toBe("Store directory changed");
});
