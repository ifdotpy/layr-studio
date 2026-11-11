// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import FilePreview, { type FilePreviewSelection } from "./FilePreview.tsx";

vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerFileActions: () => ({ canUseNativeFiles: () => false }),
  }),
);
vi.mock("../PreviewModal/PreviewModal.tsx", () => ({
  default: ({ children }: { children: React.ReactNode }) => children,
}));
vi.mock("../../shell/ImageViewer/ImageViewer.tsx", () => ({
  default: ({ src }: { src: string }) => createElement("img", { src }),
}));
const mounted = vi.hoisted(() => ({ states: new Set<{ current: unknown }>() }));
// Observe the values retained by mounted hooks without relying on garbage collection.
vi.mock("react", async (original) => {
  const react = await original<typeof import("react")>();
  return {
    ...react,
    useState<T>(initial?: T | (() => T)) {
      const state = react.useState(initial);
      const retained = react.useRef<unknown>(state[0]);
      retained.current = state[0];
      mounted.states.add(retained);
      react.useEffect(
        () => () => {
          mounted.states.delete(retained);
        },
        [],
      );
      return state;
    },
  };
});
beforeEach(() => vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true));
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  mounted.states.clear();
});

it("never mounts media with an empty URL while its Blob URL is allocated", async () => {
  const errors = vi.spyOn(console, "error").mockImplementation(() => {});
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:image");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  const data = { name: "image.png", mime: "image/png", base64: "YQ==" };
  const target = {
    target: { asset: "image" },
    slot: {
      current: { status: "live", data },
      read: async () => data,
    },
  } satisfies FilePreviewSelection;
  const host = document.createElement("div"),
    root = createRoot(host);
  await act(async () =>
    root.render(
      createElement(MantineProvider, {
        children: createElement(FilePreview, { target, onClose() {} }),
      }),
    ),
  );
  expect(
    errors.mock.calls
      .flat()
      .some(
        (value) => typeof value === "string" && value.includes("empty string"),
      ),
  ).toBe(false);
  expect(host.querySelector("img")?.getAttribute("src")).toBe("blob:image");
  await act(async () => root.unmount());
});

it("releases decoded preview state and its Blob URL on close, then reopens with a new URL", async () => {
  const allocate = vi
    .spyOn(URL, "createObjectURL")
    .mockReturnValueOnce("blob:first")
    .mockReturnValueOnce("blob:reopened");
  const revoke = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  const text = "closed preview payload";
  const data = { name: "large.txt", mime: "text/plain", base64: btoa(text) };
  const selection = {
    target: { asset: "large" },
    slot: {
      current: { status: "live", data },
      read: async () => data,
    },
  } satisfies FilePreviewSelection;
  const retainsFile = () =>
    [...mounted.states].some(
      ({ current }) =>
        typeof current === "object" &&
        current !== null &&
        "file" in current &&
        typeof current.file === "object" &&
        current.file !== null &&
        "text" in current.file &&
        current.file.text === text,
    );
  const host = document.createElement("div"),
    root = createRoot(host);
  const render = (target: FilePreviewSelection | null) =>
    act(async () => {
      root.render(
        createElement(MantineProvider, {
          children: createElement(FilePreview, { target, onClose() {} }),
        }),
      );
    });
  try {
    await render(selection);
    expect(host.textContent).toContain(text);
    expect(retainsFile()).toBe(true);
    await render(null);
    expect(revoke).toHaveBeenCalledWith("blob:first");
    expect(retainsFile()).toBe(false);
    expect(host.textContent).not.toContain(text);
    await render({ ...selection });
    expect(allocate).toHaveBeenCalledTimes(2);
    expect(host.textContent).toContain(text);
    expect(retainsFile()).toBe(true);
  } finally {
    await act(async () => root.unmount());
  }
  expect(revoke).toHaveBeenCalledWith("blob:reopened");
});
