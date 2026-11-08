import { BottomBandContext } from "../../../hooks/BottomInsetBand.tsx";
import { useLayoutEffect, useState, type ReactNode } from "react";

/** Extend backgrounds under Home without extending any control target. */
export function BottomInsetBand({ children }: { children: ReactNode }) {
  const [host] = useState(() => {
    const node = document.createElement("div");
    node.className = "viewport-bottom-band";
    return node;
  });
  const [visible, setVisible] = useState(false);
  useLayoutEffect(() => {
    const root = document.getElementById("root");
    if (!root) return;
    let frame = 0;
    let observedSidebar: Element | undefined;
    let observedWorkspace: Element | undefined;
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    const resize = new ResizeObserver(schedule);
    const content = new MutationObserver(schedule);
    let watchingContent = false;
    const update = () => {
      frame = 0;
      const shown =
        document.documentElement.hasAttribute("data-touch-input") &&
        !document.documentElement.hasAttribute("data-viewport-keyboard") &&
        Number.parseFloat(getComputedStyle(root).paddingBottom) > 0;
      if (shown && !host.isConnected) document.body.append(host);
      if (!shown) host.remove();
      setVisible(shown);
      if (!shown) {
        content.disconnect();
        watchingContent = false;
        if (observedSidebar) resize.unobserve(observedSidebar);
        if (observedWorkspace) resize.unobserve(observedWorkspace);
        observedSidebar = observedWorkspace = undefined;
        return;
      }
      if (!watchingContent) {
        // The sidebar and workspace mount, unmount or change presentation;
        // their size changes reach the ResizeObserver. Any class or style
        // change in the app would run this every frame while text streams.
        content.observe(root, {
          childList: true,
          subtree: true,
          attributes: true,
          attributeFilter: ["data-presentation"],
        });
        watchingContent = true;
      }
      const column = [
        ...root.querySelectorAll('#sidebar[data-presentation="inline"]'),
      ].find((node) => node.getBoundingClientRect().width > 0);
      if (column !== observedSidebar) {
        if (observedSidebar) resize.unobserve(observedSidebar);
        if (column) resize.observe(column);
        observedSidebar = column;
      }
      const surface = [...root.querySelectorAll(".workspace")].find(
        (node) => node.getBoundingClientRect().width > 0,
      );
      if (surface !== observedWorkspace) {
        if (observedWorkspace) resize.unobserve(observedWorkspace);
        if (surface) resize.observe(surface);
        observedWorkspace = surface;
      }
      for (const [name, element] of [
        ["workspace", surface],
        ["sidebar", column],
      ] as const) {
        const bounds = element?.getBoundingClientRect();
        host.style.setProperty(`--band-${name}-left`, `${bounds?.left || 0}px`);
        host.style.setProperty(
          `--band-${name}-width`,
          `${bounds?.width || 0}px`,
        );
      }
    };
    resize.observe(root);
    const mutation = new MutationObserver(schedule);
    mutation.observe(document.documentElement, {
      attributes: true,
      attributeFilter: [
        "style",
        "class",
        "data-touch-input",
        "data-viewport-keyboard",
      ],
    });
    window.addEventListener("resize", schedule);
    update();
    return () => {
      cancelAnimationFrame(frame);
      resize.disconnect();
      mutation.disconnect();
      content.disconnect();
      window.removeEventListener("resize", schedule);
      host.remove();
    };
  }, [host]);
  return (
    <BottomBandContext.Provider value={visible ? host : null}>
      {children}
    </BottomBandContext.Provider>
  );
}
