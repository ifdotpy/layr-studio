// @vitest-environment happy-dom
import { expect, test } from "vitest";
import { autorun, runInAction } from "mobx";
import { shellFixture } from "./shell.fixture.ts";
test("temporary panel clamping preserves the workspace width preference", () => {
  const f = shellFixture();
  const owner = f.areas.layout;
  f.areas.ui.showSection("messages");
  const measure = (width: number) =>
    runInAction(() => {
      owner.geometry = {
        host: { x: 0, y: 0, width, height: 800 },
        viewport: { x: 0, y: 0, width, height: 800 },
        rootFontPx: 16,
        shortTouchLandscape: false,
      };
    });
  owner.setPreferredPanelWidth(50);
  measure(1100);
  expect(owner.panelLayout.width).toBeLessThan(800);
  expect(owner.preferredPanelWidthRem).toBe(50);
  measure(1800);
  expect(owner.panelLayout.width).toBe(800);
  expect(owner.preferredPanelWidthRem).toBe(50);
  owner.dispose();
  f.dispose();
});

test("panel event actions update geometry immediately and persist only the wide team choice", () => {
  const f = shellFixture(),
    owner = f.areas.layout;
  owner.recordResize({
    ...owner.geometry,
    host: { ...owner.geometry.host, width: 1400 },
    viewport: { ...owner.geometry.viewport, width: 1400 },
  });
  const geometry = owner.geometry;
  const seen: string[] = [];
  const stop = autorun(() =>
    seen.push(`${owner.panelLayout.mode}:${owner.layout.teamInline}`),
  );
  f.areas.ui.setTeamPreferred(true);
  expect(seen.at(-1)).toBe("drawer:true");
  expect(f.services.api.saved("codex-team-open", false)).toBe(true);
  f.areas.ui.showSection("messages");
  expect(seen.at(-1)).toBe("inline:false");
  f.areas.ui.closeWorkspace();
  expect(seen.at(-1)).toBe("drawer:true");
  expect(f.services.api.saved("codex-team-open", false)).toBe(true);
  f.areas.ui.setTeamPreferred(false);
  expect(seen.at(-1)).toBe("drawer:false");
  expect(f.services.api.saved("codex-team-open", true)).toBe(false);
  expect(owner.geometry).toBe(geometry);
  stop();
  f.dispose();
});
