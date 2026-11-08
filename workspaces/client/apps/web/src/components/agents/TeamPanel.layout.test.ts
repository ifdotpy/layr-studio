// @vitest-environment happy-dom
import { layoutValues } from "../../layout/layout-contract.generated";
import { shellFixture } from "../../app/shell/shell.fixture.ts";
import { runInAction } from "mobx";
import { required } from "../componentTestFixtures";
import { readFileSync } from "node:fs";
import { URL as NodeURL } from "node:url";
import { expect, it } from "vitest";

it("keeps Team media decisions in parity with the CSS geometry owners", () => {
  // These are filesystem reads; Vite rewrites browser URL expressions in happy-dom.
  const layout = readFileSync(
    new NodeURL("../../design/layout.css", import.meta.url),
    "utf8",
  );
  const workspace = readFileSync(
    new NodeURL("../../workspace-layout.css", import.meta.url),
    "utf8",
  );
  const rem = (source: string, token: string) => {
    const match = source.match(new RegExp(`${token}: ([\\d.]+)rem`));
    expect(match).not.toBeNull();
    return Number(required(match?.[1]));
  };
  const dock = rem(layout, "--studio-form-max") + layoutValues.team.preferred;
  expect(layout).toContain("--studio-team-size: var(--layout-team-preferred)");
  expect(workspace).toContain(
    "--studio-sidebar-width: var(--layout-sidebar-preferred)",
  );
  const withSidebar = dock + layoutValues.sidebar.preferred;
  expect(workspace).toContain(
    `@container workspace-frame (min-width: ${dock}rem)`,
  );
  for (const hasDesktopSidebar of [false, true]) {
    const f = shellFixture();
    const owner = f.areas.layout;
    runInAction(() => {
      f.areas.sidebar.desktopCollapsed = !hasDesktopSidebar;
    });
    const threshold = hasDesktopSidebar ? withSidebar : dock;
    for (const rootFontPx of [16, 24]) {
      for (const offset of [-1, 0, 1]) {
        owner.recordResize({
          host: { x: 0, y: 0, width: 4000, height: 800 },
          viewport: {
            x: 0,
            y: 0,
            width: threshold * rootFontPx + offset,
            height: 800,
          },
          rootFontPx,
          shortTouchLandscape: false,
        });
        expect(owner.isNarrowTeam).toBe(offset < 0);
      }
    }
    f.dispose();
  }
});
