import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test, vi } from "vitest";
const renderWarnings = vi.hoisted(() => vi.fn(() => null));
vi.mock("./ConversationWarnings", async () => ({
  default: (await import("mobx-react-lite")).observer(renderWarnings),
}));
import SidebarWarnings from "./SidebarWarnings.tsx";

test("an unattached sidebar warning target acquires no transcript interest", () => {
  renderToStaticMarkup(createElement(SidebarWarnings));
  expect(renderWarnings).not.toHaveBeenCalled();
});
