import { Drawer, type DrawerProps } from "@mantine/core";
import type { ReactNode } from "react";
import { useSheetSwipe } from "./useSheetSwipe.ts";

/**
 * The phone presentation of a popover: a bottom sheet up to three quarters
 * of the screen. The backdrop above it closes it on a tap, and its title row
 * is a grip that closes it with a downward swipe.
 */
export default function BottomSheet({
  title,
  onClose,
  className,
  children,
  ...drawer
}: Omit<DrawerProps, "position" | "size" | "title"> & {
  title: ReactNode;
  children: ReactNode;
}) {
  const swipe = useSheetSwipe(onClose);
  return (
    <Drawer
      {...drawer}
      onClose={onClose}
      position="bottom"
      size="auto"
      withinPortal
      className={`bottom-sheet${className ? ` ${className}` : ""}`}
      title={
        <span className="sheet-grip" {...swipe}>
          {title}
        </span>
      }
    >
      {children}
    </Drawer>
  );
}
