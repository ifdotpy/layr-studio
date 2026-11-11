import type { ServerAreas } from "../contracts.ts";
import {
  compareStructural,
  makeAutoObservable,
  observableRef,
  reaction,
  action,
} from "mobx";
import {
  layoutValues,
  layoutQueries,
} from "../../layout/layout-contract.generated.ts";
import {
  deriveWorkspaceLayout,
  isCompactTools,
  type LayoutRect,
  type WorkspaceLayoutInput,
} from "../../layout/deriveWorkspaceLayout.ts";

import { observeEdgeRail } from "../../layout/useEdgeRail.ts";
import type { EdgeRailGeometry } from "../../layout/edgeRailPolicy.ts";
import { attachMobileViewport } from "../../hooks/mobileViewport.ts";
const emptyRect: LayoutRect = { x: 0, y: 0, width: 0, height: 0 };
export class LayoutOwner {
  geometry: WorkspaceLayoutInput & { shortTouchLandscape: boolean } = {
    host: emptyRect,
    viewport: emptyRect,
    rootFontPx: layoutValues.viewport.rootFontDefault,
    shortTouchLandscape: false,
  };
  isSettingsFullscreen = false;
  workspaceWidth = 0;
  mainFontPx = 0;
  workspace: HTMLElement | null = null;
  preferredPanelWidthRem: number | undefined;
  host: HTMLElement | null = null;
  row: HTMLDivElement | null = null;
  slot: HTMLDivElement | null = null;
  stopRow: (() => void) | undefined;
  stopViewport: (() => void) | undefined;
  railGeometry: EdgeRailGeometry | null = null;
  readonly areas: ServerAreas;
  constructor(areas: LayoutOwner["areas"]) {
    this.areas = areas;
    makeAutoObservable(
      this,
      {
        areas: false,
        workspace: observableRef,
        row: observableRef,
        host: observableRef,
        slot: observableRef,
        stopRow: false,
        stopViewport: false,
        railGeometry: observableRef,
      },
      { autoBind: true },
    );
  }
  get isMobile() {
    return this.geometry.viewport.width < layoutValues.navigation.inlineMin;
  }
  get layout() {
    return deriveWorkspaceLayout({
      ...this.geometry,
      workspaceOpen: this.areas.ui.opened,
      teamPreferred: this.areas.ui.teamPreferred,
      ...(this.preferredPanelWidthRem === undefined
        ? {}
        : { preferredPanelWidthRem: this.preferredPanelWidthRem }),
      hostExcludesSafeInsets: true,
    });
  }
  get panelLayout() {
    return this.layout.panelLayout;
  }
  get isNarrowTeam() {
    const required =
      layoutValues.conversation.min +
      layoutValues.team.preferred +
      (this.areas.sidebar.hasDesktopSidebar
        ? layoutValues.sidebar.preferred
        : 0);
    return !(
      this.layout.teamCanInline &&
      this.geometry.viewport.width >= required * this.geometry.rootFontPx
    );
  }
  get isCompactHeader() {
    return this.geometry.shortTouchLandscape && !this.railGeometry;
  }
  get usesCompactConversationTools() {
    return (
      this.isMobile ||
      isCompactTools(
        this.workspaceWidth || this.geometry.viewport.width,
        this.mainFontPx,
      )
    );
  }
  attachWorkspace(node: HTMLElement | null) {
    this.workspace = node;
    this.attachRow(this.row);
  }
  get usesMenuHeaderTools() {
    return !!this.railGeometry || this.isCompactHeader;
  }
  recordResize(geometry: LayoutOwner["geometry"]) {
    if (!compareStructural(this.geometry, geometry)) this.geometry = geometry;
  }
  setPreferredPanelWidth(width: number | undefined) {
    if (width === undefined || (Number.isFinite(width) && width > 0))
      this.preferredPanelWidthRem = width;
  }
  attachHost(host: HTMLElement | null) {
    this.host = host;
  }
  attachSlot(node: HTMLDivElement | null) {
    this.slot = node;
  }
  attachRow(host: HTMLDivElement | null) {
    this.stopRow?.();
    this.row = host;
    this.stopRow = host ? this.observeRow(host) : undefined;
  }
  start() {
    this.stopViewport?.();
    if (!this.row) this.stopRow = this.observeRow(document.documentElement);
    const settingsQuery = window.matchMedia(layoutQueries.settingsFullScreen);
    const measureSettings = action(() => {
      this.isSettingsFullscreen = settingsQuery.matches;
    });
    measureSettings();
    settingsQuery.addEventListener("change", measureSettings);
    const stopScope = reaction(
      () => this.areas.startup.creationScope,
      () => {
        this.preferredPanelWidthRem = undefined;
      },
    );
    let detach: (() => void) | undefined;
    const stop = reaction(
      () => [this.areas.scope.screen.active, this.isMobile] as const,
      ([active, mobile]) => {
        detach?.();
        const viewport = active ? attachMobileViewport(mobile) : undefined;
        const rail = active ? observeEdgeRail(this.recordRail) : undefined;
        if (!active) this.railGeometry = null;
        detach = () => {
          viewport?.();
          rail?.();
        };
      },
      { fireImmediately: true },
    );
    this.stopViewport = () => {
      stop();
      stopScope();
      settingsQuery.removeEventListener("change", measureSettings);
      detach?.();
    };
    return this.dispose;
  }
  dispose() {
    this.stopViewport?.();
    this.stopViewport = undefined;
    this.attachRow(null);
    this.slot = null;
  }
  recordRail(geometry: EdgeRailGeometry | null) {
    this.railGeometry = geometry;
  }
  private observeRow(host: HTMLElement) {
    const document = host.ownerDocument;
    const view = document.defaultView!;
    const root = document.documentElement;
    // Root style writes come every frame during drags (custom properties);
    // only these inputs change the root font size or the landscape flag.
    const inputs = () =>
      `${root.className}|${root.style.fontSize}|${root.getAttribute("data-viewport-short-landscape")}|${root.dataset.studioTypography}|${root.style.getPropertyValue("--studio-main-font-size")}`;
    let measured: string | undefined;
    let rootFontPx = Number.NaN;
    let frame = 0;
    const update = action(() => {
      frame = 0;
      const key = inputs();
      if (key !== measured) {
        measured = key;
        const style = view.getComputedStyle(root);
        rootFontPx = Number.parseFloat(style.fontSize);
        this.mainFontPx = Number.parseFloat(
          style.getPropertyValue("--studio-main-font-size"),
        );
      }
      const bounds = host.getBoundingClientRect();
      this.workspaceWidth =
        this.workspace?.getBoundingClientRect().width ?? view.innerWidth;
      const next = {
        host: {
          x: bounds.x,
          y: bounds.y,
          width: bounds.width,
          height: bounds.height,
        },
        // The viewport hook owns keyboard and safe-area adjustments to the root.
        viewport: {
          x: 0,
          y: 0,
          width: view.innerWidth,
          height: view.innerHeight,
        },
        rootFontPx,
        shortTouchLandscape: root.hasAttribute("data-viewport-short-landscape"),
      };
      this.recordResize(next);
    });
    // One measurement per frame, after the frame's own changes.
    const schedule = () => {
      if (!frame) frame = view.requestAnimationFrame(update);
    };
    const resize = new ResizeObserver(schedule);
    resize.observe(host);
    resize.observe(root);
    if (this.workspace) resize.observe(this.workspace);
    const preferences = new MutationObserver(() => {
      if (inputs() !== measured) schedule();
    });
    preferences.observe(root, {
      attributes: true,
      attributeFilter: [
        "style",
        "class",
        "data-viewport-short-landscape",
        "data-studio-typography",
      ],
    });
    view.addEventListener("resize", schedule);
    update();
    return () => {
      view.cancelAnimationFrame(frame);
      resize.disconnect();
      preferences.disconnect();
      view.removeEventListener("resize", schedule);
    };
  }
}
