import { Drawer, FocusTrap } from "@mantine/core";
import {
  action,
  makeAutoObservable,
  observableRef,
  observableStruct,
  reaction,
} from "mobx";
import { observer } from "mobx-react-lite";
import {
  useLayoutEffect,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
} from "react";
import type { SidebarListOwner } from "../../../app/shell/SidebarOwner.ts";
import {
  layoutQueries,
  layoutValues,
} from "../../../layout/layout-contract.generated.ts";
import { useSidebarState } from "../support/SidebarState.tsx";
import { SidebarResizeSession } from "./sidebarResize.ts";
import {
  clampSidebarWidth,
  resolveSidebarWidth,
  storedSidebarWidth,
} from "./sidebarWidth.ts";

const widthContract = {
  minimumRem: layoutValues.sidebar.min,
  preferredRem: layoutValues.sidebar.preferred,
  conversationMinRem: layoutValues.conversation.min,
  navigationInlineMinPx: layoutValues.navigation.inlineMin,
};
/** Move focus to the visible control before removing sidebar content. */
export function restoreSidebarFocus() {
  const sidebar = document.getElementById("sidebar");
  if (!sidebar?.contains(document.activeElement)) return;
  document
    .querySelector<HTMLElement>("#sidebar-toggle, .server-mobile-toggle")
    ?.focus({ preventScroll: true });
}

/** Sole width authority; pointer drafts touch the sidebar and dock, never document styles. */
export class SidebarFrameOwner {
  readonly source: SidebarListOwner;
  aside: HTMLElement | null = null;
  headerMinimum = 0;
  scrollTop = 0;
  geometry = { hostWidth: 0, rootFontSize: 0, headerMinimum: 0 };
  preference: { scope: string; width: number | null } | null = null;
  draft: number | null = null;
  readonly session = new SidebarResizeSession();
  constructor(source: SidebarListOwner) {
    this.source = source;
    makeAutoObservable(
      this,
      {
        source: false,
        aside: observableRef,
        session: false,
        geometry: observableStruct,
        preference: observableRef,
        scrollTop: false,
      },
      { autoBind: true },
    );
  }
  get scope() {
    return `${this.source.services.cache.ownerId}:${this.source.stateDir}`;
  }
  get preferred() {
    return this.preference?.scope === this.scope ? this.preference.width : null;
  }
  get policy() {
    return resolveSidebarWidth({
      ...this.geometry,
      preferred: this.preferred,
      contract: widthContract,
    });
  }
  get compact() {
    return typeof window !== "undefined" && this.policy.mode === "drawer";
  }
  get probing() {
    return (
      !this.compact &&
      this.source.visibility.desktopCollapsed &&
      this.headerMinimum === 0
    );
  }
  get enabled() {
    return !this.compact && !this.source.visibility.desktopCollapsed;
  }
  get width() {
    return this.draft === null
      ? this.policy.width
      : clampSidebarWidth(this.draft, this.policy.minimum, this.policy.maximum);
  }
  get step() {
    return layoutValues.resize.step * this.geometry.rootFontSize;
  }
  node(node: HTMLElement | null) {
    this.aside = node;
  }
  save(width: number | null) {
    if (width !== null && storedSidebarWidth(width) === null) return;
    this.preference = { scope: this.scope, width };
  }
  reset() {
    this.cancel();
    this.save(null);
  }
  cancel() {
    this.session.cancel();
    this.draft = null;
  }
  get handlers() {
    return {
      onPointerDown: this.pointerDown,
      onPointerMove: this.pointerMove,
      onPointerUp: this.pointerUp,
      onPointerCancel: this.cancel,
      onLostPointerCapture: this.cancel,
      onDoubleClick: this.reset,
      onKeyDown: this.keyDown,
    };
  }
  pointerDown(event: PointerEvent<HTMLElement>) {
    if (!this.enabled || event.button !== 0 || !event.isPrimary) return;
    event.preventDefault();
    event.currentTarget.focus();
    event.currentTarget.setPointerCapture(event.pointerId);
    this.session.start(
      this.scope,
      event.pointerId,
      event.clientX,
      this.policy.width,
    );
  }
  pointerMove(event: PointerEvent<HTMLElement>) {
    const width = this.session.move(
      this.scope,
      event.pointerId,
      event.clientX,
      this.policy,
    );
    if (width !== null) this.draft = width;
  }
  pointerUp(event: PointerEvent<HTMLElement>) {
    const width = this.session.finish(
      this.scope,
      event.pointerId,
      event.clientX,
      this.policy,
    );
    if (width !== null) {
      this.draft = null;
      this.save(width);
    }
    if (event.currentTarget.hasPointerCapture(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId);
  }
  keyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      this.cancel();
      return;
    }
    if (["Home", "Enter"].includes(event.key)) {
      event.preventDefault();
      this.reset();
    } else if (["ArrowLeft", "ArrowRight", "End"].includes(event.key)) {
      event.preventDefault();
      this.cancel();
      this.save(
        event.key === "End"
          ? this.policy.maximum
          : clampSidebarWidth(
              this.policy.width +
                (event.key === "ArrowRight" ? this.step : -this.step),
              this.policy.minimum,
              this.policy.maximum,
            ),
      );
    }
  }
  attach() {
    const stopMeasurement = this.measureHost();
    const stopSession = reaction(() => [this.scope, this.enabled], this.cancel);
    const escape = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") this.cancel();
    };
    window.addEventListener("keydown", escape, true);
    window.addEventListener("blur", this.cancel);
    const root = document.getElementById("root") ?? document.documentElement;
    const previous = root.style.getPropertyValue("--studio-sidebar-width");
    const stopWidth = reaction(
      () => [this.enabled, this.policy.width] as const,
      ([enabled, width]) => {
        if (enabled)
          root.style.setProperty("--studio-sidebar-width", width + "px");
        else if (previous)
          root.style.setProperty("--studio-sidebar-width", previous);
        else root.style.removeProperty("--studio-sidebar-width");
      },
      { fireImmediately: true },
    );
    const stopDraft = reaction(
      () => (this.draft === null ? null : this.width),
      (width) => {
        const dock = document.querySelector<HTMLElement>(
          "#root .terminal-dock",
        );
        if (width === null)
          dock?.style.removeProperty("--studio-sidebar-width");
        else dock?.style.setProperty("--studio-sidebar-width", width + "px");
      },
    );
    return () => {
      stopMeasurement?.();
      stopSession();
      stopWidth();
      stopDraft();
      this.cancel();
      window.removeEventListener("keydown", escape, true);
      window.removeEventListener("blur", this.cancel);
      if (previous) root.style.setProperty("--studio-sidebar-width", previous);
      else root.style.removeProperty("--studio-sidebar-width");
      document
        .querySelector<HTMLElement>("#root .terminal-dock")
        ?.style.removeProperty("--studio-sidebar-width");
    };
  }
  measureHost() {
    const host =
      document.querySelector<HTMLElement>(".multi-server-shell") ||
      document.getElementById("root");
    if (!host) return;
    // The header minimum depends on its content and typography, not on the
    // sidebar width. Measuring it forces layout several times, so it runs when
    // those inputs change, never for each frame of a width drag. The inputs
    // are read from the inline root style and classes, which forces nothing.
    const typography = () => {
      const root = document.documentElement;
      return [
        root.className,
        root.dataset.studioTypography,
        root.style.fontSize,
        root.style.getPropertyValue("--studio-sidebar-font-size"),
        root.style.getPropertyValue("--studio-font-family"),
        document.body.className,
      ].join("|");
    };
    let measuredTypography: string | undefined;
    const measure = action((headerChanged = true) => {
      measuredTypography = typography();
      const rootFontSize = Number.parseFloat(
        getComputedStyle(document.documentElement).fontSize,
      );
      const hostStyle = getComputedStyle(host);
      const hostWidth =
        host.clientWidth -
        parseFloat(hostStyle.paddingLeft) -
        parseFloat(hostStyle.paddingRight);
      const header = this.aside?.querySelector<HTMLElement>(".sidebar-header");
      // Drawer touch controls must not raise the desktop header minimum.
      if (
        headerChanged &&
        header &&
        this.aside &&
        !this.compact &&
        window.matchMedia(layoutQueries.navigationInline).matches
      ) {
        const style = getComputedStyle(header);
        const sideStyle = getComputedStyle(this.aside);
        const children = Array.from(header.children) as HTMLElement[];
        const minimumWidths = children.map((child) => {
          if (Number(getComputedStyle(child).flexGrow) === 0)
            return child.getBoundingClientRect().width;
          // A flexible label can wrap; its expanded width is not its minimum.
          const previous = child.style.cssText;
          try {
            child.style.flex = "none";
            child.style.inlineSize = "min-content";
            return child.getBoundingClientRect().width;
          } finally {
            child.style.cssText = previous;
          }
        });
        const contentMinimum =
          style.flexWrap === "nowrap"
            ? minimumWidths.reduce((sum, width) => sum + width, 0) +
              Math.max(0, children.length - 1) *
                (parseFloat(style.columnGap) || 0)
            : Math.max(0, ...minimumWidths);
        this.headerMinimum =
          contentMinimum +
          parseFloat(sideStyle.paddingLeft) +
          parseFloat(sideStyle.paddingRight) +
          parseFloat(sideStyle.borderLeftWidth) +
          parseFloat(sideStyle.borderRightWidth);
      }
      const next = {
        hostWidth,
        rootFontSize,
        headerMinimum: this.headerMinimum,
      };
      const nextCompact =
        resolveSidebarWidth({
          ...next,
          preferred: null,
          contract: widthContract,
        }).mode === "drawer";
      if (nextCompact !== this.compact) restoreSidebarFocus();
      this.geometry = next;
      this.source.visibility.recordCompact(this.compact);
    });
    const remeasure = () => measure();
    // The host width decides the layout mode; a resize keeps the header minimum.
    const observer = new ResizeObserver(() => measure(false));
    observer.observe(host);
    const content = new MutationObserver(remeasure);
    const header = this.aside?.querySelector(".sidebar-header");
    if (header)
      content.observe(header, {
        subtree: true,
        childList: true,
        characterData: true,
        attributes: true,
        // Not "style": measuring sets and restores the children's style.
        attributeFilter: ["class", "hidden"],
      });
    const attributes = new MutationObserver(() => {
      if (typography() !== measuredTypography) remeasure();
    });
    attributes.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["style", "class", "data-studio-typography"],
    });
    attributes.observe(document.body, {
      attributes: true,
      attributeFilter: ["style", "class"],
    });
    document.fonts?.addEventListener("loadingdone", remeasure);
    window.addEventListener("resize", remeasure);
    measure();
    return () => {
      observer.disconnect();
      content.disconnect();
      attributes.disconnect();
      document.fonts?.removeEventListener("loadingdone", remeasure);
      window.removeEventListener("resize", remeasure);
    };
  }
}

export const SidebarFrame = observer(function SidebarFrame({
  children,
  drawerZIndex,
}: {
  children: ReactNode;
  drawerZIndex?: number;
}) {
  const { frame } = useSidebarState();
  useLayoutEffect(() => {
    const stop = frame.attach();
    const list = frame.aside?.querySelector<HTMLElement>("#chat-list");
    if (list) list.scrollTop = frame.scrollTop;
    return () => {
      if (list) frame.scrollTop = list.scrollTop;
      stop();
      restoreSidebarFocus();
    };
  }, [
    frame,
    frame.compact,
    frame.source.visibility.desktopCollapsed,
    frame.source.visibility.mobileOpen,
  ]);
  const content = (
    <aside
      ref={frame.node}
      id="sidebar"
      className="sidebar-frame"
      data-presentation={frame.compact ? "drawer" : "inline"}
      data-probe={frame.probing || undefined}
      inert={frame.probing || undefined}
      aria-hidden={frame.probing || undefined}
      aria-label="Conversations"
      style={
        frame.compact
          ? undefined
          : { width: frame.width, flexBasis: frame.width }
      }
    >
      {children}
      {!frame.compact && (
        <div
          className="sidebar-resize-rail"
          role="separator"
          tabIndex={0}
          aria-label="Conversation sidebar width"
          aria-orientation="vertical"
          aria-controls="sidebar"
          aria-valuemin={Math.ceil(frame.policy.minimum)}
          aria-valuemax={Math.floor(frame.policy.maximum)}
          aria-valuenow={Math.round(frame.width)}
          aria-valuetext={`${Math.round(frame.width)} pixels`}
          title="Drag or use arrow keys to resize. Press Home to reset."
          {...frame.handlers}
        />
      )}
    </aside>
  );
  if (!frame.compact)
    return frame.source.visibility.desktopCollapsed && !frame.probing
      ? null
      : content;
  return (
    <Drawer
      opened={frame.source.visibility.mobileOpen}
      onEnterTransitionEnd={frame.source.focusSelected}
      onClose={frame.source.closeSidebar}
      {...(drawerZIndex !== undefined ? { zIndex: drawerZIndex } : {})}
      size={`min(${layoutValues.sidebar.drawerMax}rem, 100vw)`}
      padding={0}
      withCloseButton={false}
      title="Conversations"
      classNames={{ header: "sr-only", body: "sidebar-drawer-body" }}
    >
      {/* Focus enters the drawer without ringing its first link. */}
      <FocusTrap.InitialFocus />
      {content}
    </Drawer>
  );
});
