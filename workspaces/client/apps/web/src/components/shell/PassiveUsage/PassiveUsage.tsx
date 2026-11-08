import { BottomBandContext } from "../../../hooks/BottomInsetBand.tsx";
import {
  useContext,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

/** Only passive session text belongs in this portal. Keep buttons in Usage. */
export function PassiveUsage({ children }: { children: ReactNode }) {
  const host = useContext(BottomBandContext);
  const anchor = useRef<HTMLSpanElement>(null);
  const [rect, setRect] = useState({ left: 0, width: 0 });
  useLayoutEffect(() => {
    const footer = anchor.current?.parentElement;
    if (!host || !footer) return;
    const update = () => {
      const bounds = footer.getBoundingClientRect();
      setRect((old) =>
        old.left === bounds.left && old.width === bounds.width
          ? old
          : { left: bounds.left, width: bounds.width },
      );
    };
    const resize = new ResizeObserver(update);
    resize.observe(footer);
    window.addEventListener("resize", update);
    update();
    return () => {
      resize.disconnect();
      window.removeEventListener("resize", update);
    };
  }, [host]);
  return (
    <span className="usage-passive-anchor" ref={anchor}>
      {host
        ? createPortal(
            <div className="usage-passive-band" style={rect}>
              {children}
            </div>,
            host,
          )
        : children}
    </span>
  );
}
