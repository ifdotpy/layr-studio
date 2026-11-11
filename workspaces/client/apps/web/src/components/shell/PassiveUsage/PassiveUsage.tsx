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
  const [band, setBand] = useState<HTMLDivElement | null>(null);
  useLayoutEffect(() => {
    const footer = anchor.current?.parentElement;
    const element = band;
    if (!host || !footer || !element) return;
    const update = () => {
      const bounds = footer.getBoundingClientRect();
      element.style.left = `${bounds.left}px`;
      element.style.width = `${bounds.width}px`;
    };
    const resize = new ResizeObserver(update);
    resize.observe(footer);
    window.addEventListener("resize", update);
    update();
    return () => {
      resize.disconnect();
      window.removeEventListener("resize", update);
    };
  }, [host, band]);
  return (
    <span className="usage-passive-anchor" ref={anchor}>
      {host
        ? createPortal(
            <div className="usage-passive-band" ref={setBand}>
              {children}
            </div>,
            host,
          )
        : children}
    </span>
  );
}
