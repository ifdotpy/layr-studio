// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const h = vi.hoisted(() => ({ native: false, pick: vi.fn(), change: vi.fn() }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerNet: () => ({}),
    useServerFileDialogs: () => ({
      canPickDirectory: () => h.native,
      pickDirectory: h.pick,
    }),
  }),
);
vi.mock("../ProjectDirectoryPicker/ProjectDirectoryPicker.tsx", () => ({
  default: ({ onSelect }: { onSelect: (path: string) => Promise<void> }) =>
    createElement(
      "button",
      { onClick: () => void onSelect("/server") },
      "Use server folder",
    ),
  browseDirectory: () => ({}),
}));
import DirectoryField from "./DirectoryField.tsx";
let host: HTMLDivElement;
let root: ReturnType<typeof createRoot>;
beforeEach(async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  h.native = false;
  h.pick.mockReset();
  h.change.mockReset();
  host = document.body.appendChild(document.createElement("div"));
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.unstubAllGlobals();
});
async function choose() {
  await act(async () =>
    root.render(
      createElement(
        MantineProvider,
        null,
        createElement(DirectoryField, {
          label: "Folder",
          value: "/old",
          onChange: h.change,
        }),
      ),
    ),
  );
  const button = host.querySelector<HTMLButtonElement>(
    '[aria-label="Choose folder"]',
  );
  if (!button) throw Error("Missing folder chooser");
  await act(async () => button.click());
}
it("uses the native dialog when available", async () => {
  h.native = true;
  h.pick.mockResolvedValue("/native");
  await choose();
  expect(h.change).toHaveBeenCalledWith("/native");
});
it("uses the server picker otherwise", async () => {
  await choose();
  const button = [...document.querySelectorAll("button")].find(
    (node) => node.textContent === "Use server folder",
  );
  if (!button) throw Error("Missing server picker");
  await act(async () => button.click());
  expect(h.change).toHaveBeenCalledWith("/server");
  expect(h.pick).not.toHaveBeenCalled();
});
it("does not update an unmounted field when its native picker finishes", async () => {
  let finish: (value: string) => void = () => {};
  h.native = true;
  h.pick.mockReturnValue(
    new Promise<string>((resolve) => {
      finish = resolve;
    }),
  );
  await choose();
  await act(async () => root.render(null));
  await act(async () => finish("/late"));
  expect(h.change).not.toHaveBeenCalled();
});
