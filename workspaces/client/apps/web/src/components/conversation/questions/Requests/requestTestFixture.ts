import { act, createElement, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, vi } from "vitest";
import {
  RequestArea,
  RequestAreaOwner,
  type RequestCommands,
  type RequestDependencies,
} from "../RequestArea/RequestArea.tsx";

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

export function requestTestRoot(
  dispose: () => void,
  dependencies: () => RequestDependencies,
) {
  let owner: RequestAreaOwner | undefined;
  let current: RequestCommands;
  const root = createRoot(
    document.body.appendChild(document.createElement("div")),
  );
  return {
    async render(children: ReactNode, commands: RequestCommands) {
      if (owner && current.scope !== commands.scope) {
        owner.dispose();
        owner = undefined;
      }
      current = commands;
      owner ??= new RequestAreaOwner(dependencies(), () => current);
      owner.start();
      const selected = owner;
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            null,
            createElement(RequestArea, { owner: selected }, children),
          ),
        ),
      );
    },
    async close() {
      await act(async () => root.unmount());
      owner?.dispose();
      dispose();
    },
  };
}
