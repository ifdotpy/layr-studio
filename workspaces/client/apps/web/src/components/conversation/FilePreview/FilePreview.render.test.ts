// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import FilePreview, {
  type FilePreviewSelection,
  type PreviewSource,
} from "./FilePreview.tsx";
import { onBecomeUnobserved, makeAutoObservable, action } from "mobx";
import { FilePreviewState } from "../support/FilePreviewState.ts";

const prepare = (source: PreviewSource): FilePreviewSelection => ({
  ...source,
  state: new FilePreviewState(source),
});

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
beforeEach(() => vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true));
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it("never mounts media with an empty URL while its Blob URL is allocated", async () => {
  const errors = vi.spyOn(console, "error").mockImplementation(() => {});
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:image");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  const data = { name: "image.png", mime: "image/png", base64: "YQ==" };
  const target = prepare({
    target: { asset: "image" },
    slot: {
      current: { status: "live", data },
      read: async () => data,
    },
  });
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
  const errors = vi.spyOn(console, "error").mockImplementation(() => {});
  const allocate = vi
    .spyOn(URL, "createObjectURL")
    .mockReturnValueOnce("blob:first")
    .mockReturnValueOnce("blob:reopened");
  const revoke = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  const text = "closed preview payload";
  const data = { name: "large.png", mime: "image/png", base64: btoa(text) };
  const selection = prepare({
    target: { asset: "large" },
    slot: {
      current: { status: "live", data },
      read: async () => data,
    },
  });
  const released = vi.fn();
  const stop = onBecomeUnobserved(selection.state, "file", released);
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
    expect(host.querySelector("img")).not.toBeNull();
    await render(null);
    expect(revoke).toHaveBeenCalledWith("blob:first");
    expect(released).toHaveBeenCalledOnce();
    expect(host.textContent).not.toContain(text);
    await render(prepare(selection));
    expect(allocate).toHaveBeenCalledTimes(2);
    expect(host.querySelector("img")).not.toBeNull();
  } finally {
    await act(async () => root.unmount());
  }
  stop();
  expect(revoke).toHaveBeenCalledWith("blob:reopened");
  expect(errors).not.toHaveBeenCalled();
});

it("clears revoked media and exposes invalid SVG failures", async () => {
  const errors = vi.spyOn(console, "error").mockImplementation(() => {});
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:valid");
  const revoke = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  const data = { name: "image.png", mime: "image/png", base64: "QQ==" };
  const slot = makeAutoObservable({
    current: { status: "live" as const, data },
    read: async () => data,
  });
  const target = prepare({ target: { asset: "image" }, slot });
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(MantineProvider, {
          children: createElement(FilePreview, { target, onClose() {} }),
        }),
      ),
    );
    expect(host.querySelector("img")?.getAttribute("src")).toBe("blob:valid");
    await act(async () =>
      action(() => {
        slot.current.data = {
          name: "bad.svg",
          mime: "image/svg+xml",
          base64: btoa("not SVG"),
        };
      })(),
    );
    expect(revoke).toHaveBeenCalledWith("blob:valid");
    expect(host.querySelector("img")).toBeNull();
    expect(host.textContent).toContain("This SVG file is invalid.");
    expect(errors).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
  }
});

it("shows text without allocating a media resource", async () => {
  const allocate = vi.spyOn(URL, "createObjectURL");
  const data = {
    name: "note.txt",
    mime: "text/plain",
    base64: btoa("plain source"),
  };
  const target = prepare({
    target: { asset: "text" },
    slot: { current: { status: "live", data }, read: async () => data },
  });
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(MantineProvider, {
          children: createElement(FilePreview, { target, onClose() {} }),
        }),
      ),
    );
    expect(host.textContent).toContain("plain source");
    expect(allocate).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
  }
});
