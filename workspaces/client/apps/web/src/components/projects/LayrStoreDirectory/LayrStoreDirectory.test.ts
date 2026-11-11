// @vitest-environment happy-dom
import { act, createElement, type ReactNode, type ComponentProps } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { LayrStoreDirectory } from "./LayrStoreDirectory.tsx";

const h = vi.hoisted(() => ({
  save: vi.fn(),
  change: (_value: string) => {},
}));
vi.mock("@mantine/core", async () => {
  const { createElement } = await import("react");
  const container = ({ children }: { children?: ReactNode }) =>
    createElement("div", null, children);
  return {
    Group: container,
    Text: container,
    Button: ({
      children,
      disabled,
      onClick,
    }: {
      children?: ReactNode;
      disabled?: boolean;
      onClick?: () => void;
    }) => createElement("button", { disabled, onClick }, children),
  };
});
vi.mock("../DirectoryField/DirectoryField.tsx", async () => {
  const { createElement } = await import("react");
  return {
    default: ({
      value,
      onChange,
    }: {
      value: string;
      onChange: (value: string) => void;
    }) => {
      h.change = onChange;
      return createElement("input", { value, readOnly: true });
    },
  };
});
let host: HTMLDivElement;
let root: ReturnType<typeof createRoot>;
function button(label: string) {
  const found = [...host.querySelectorAll("button")].find(
    (button) => button.textContent === label,
  );
  if (!found) throw new Error(`Missing ${label}`);
  return found;
}
type Source = ComponentProps<typeof LayrStoreDirectory>["source"];
async function render(path = "/current", source: Source = "project") {
  await act(async () =>
    root.render(
      createElement(LayrStoreDirectory, {
        label: "Store",
        path,
        source,
        overridden: true,
        save: h.save,
      }),
    ),
  );
}
async function click(label: string) {
  await act(async () => button(label).click());
}
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  h.save.mockReset().mockResolvedValue(undefined);
  host = document.body.appendChild(document.createElement("div"));
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.unstubAllGlobals();
});
it("saves the chosen or typed directory", async () => {
  await render();
  await act(async () => h.change("/new"));
  await click("Save");
  expect(h.save).toHaveBeenCalledWith("/new");
});
it("persists null on reset", async () => {
  await render();
  await click("Reset");
  expect(h.save).toHaveBeenCalledWith(null);
});
it("shows save failures", async () => {
  h.save.mockRejectedValue(new Error("Store directory changed"));
  await render();
  await click("Reset");
  expect(host.querySelector('[role="alert"]')?.textContent).toBe(
    "Store directory changed",
  );
});
const sources: Source[] = ["project", "default"];
it.each(sources)(
  "discards the %s draft after a persisted A → B → A save and reset",
  async (source) => {
    await render("/A", source);
    await act(async () => h.change("/B"));
    await click("Save");
    expect(h.save).toHaveBeenLastCalledWith("/B");
    await render("/B", source);
    await click("Reset");
    expect(h.save).toHaveBeenLastCalledWith(null);
    await render("/A", source);
    expect(host.querySelector("input")?.value).toBe("/A");
    expect(button("Save").disabled).toBe(true);
  },
);
it("uses a replacement save callback when the effective path is unchanged", async () => {
  await render();
  const nextSave = vi.fn().mockResolvedValue(undefined);
  await act(async () =>
    root.render(
      createElement(LayrStoreDirectory, {
        label: "Store",
        path: "/current",
        source: "project",
        overridden: true,
        save: nextSave,
      }),
    ),
  );
  await act(async () => h.change("/replacement"));
  await click("Save");
  expect(nextSave).toHaveBeenCalledWith("/replacement");
  expect(h.save).not.toHaveBeenCalled();
});
