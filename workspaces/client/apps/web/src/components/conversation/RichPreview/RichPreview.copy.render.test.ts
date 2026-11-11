// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import RichPreview from "./RichPreview.tsx";

const clipboard = vi.hoisted(() =>
  vi.fn<(...args: unknown[]) => Promise<void>>(),
);
vi.mock("../../../clipboard/clipboard", () => ({ copyText: clipboard }));

it.each(["success", "failure", "pending-success", "pending-failure"])(
  "keeps %s copy feedback with its source",
  async (mode) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    const pending = Promise.withResolvers<void>();
    clipboard.mockImplementation(() =>
      mode.startsWith("pending")
        ? pending.promise
        : mode === "failure"
          ? Promise.reject(new Error("blocked"))
          : Promise.resolve(),
    );
    const host = document.createElement("div"),
      root = createRoot(host);
    const render = (source: string) =>
      act(async () =>
        root.render(
          createElement(MantineProvider, {
            children: createElement(RichPreview, { kind: "html", source }),
          }),
        ),
      );
    try {
      await render("A");
      await act(async () =>
        host
          .querySelector<HTMLButtonElement>('[aria-label="Copy source"]')
          ?.click(),
      );
      await render("B");
      await act(async () =>
        mode === "pending-failure"
          ? pending.reject(new Error("blocked"))
          : pending.resolve(),
      );
      expect(
        host.querySelector('[aria-label="Copy source"] .lucide-check'),
      ).toBeNull();
      expect(host.querySelector('[role="alert"]')).toBeNull();
    } finally {
      await act(async () => root.unmount());
      vi.unstubAllGlobals();
    }
  },
);
