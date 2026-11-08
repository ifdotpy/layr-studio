import { act, createElement, type ComponentProps, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, vi } from "vitest";
import { RequestArea } from "./Requests.tsx";

export function requestTestEnvironment() {
  beforeEach(() => {
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
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: { addEventListener() {}, removeEventListener() {} },
    });
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    document.body.replaceChildren();
  });
}

export function requestTestRoot(dispose: () => void) {
  const root = createRoot(
    document.body.appendChild(document.createElement("div")),
  );
  return {
    async render(
      children: ReactNode,
      commands: Omit<ComponentProps<typeof RequestArea>, "children">,
    ) {
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            null,
            createElement(RequestArea, commands, children),
          ),
        ),
      );
    },
    async close() {
      await act(async () => root.unmount());
      dispose();
    },
  };
}
