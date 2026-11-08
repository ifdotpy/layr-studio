import type { ReactNode } from "react";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const LayoutHost = observer(function LayoutHost({
  children,
}: {
  children: ReactNode;
}) {
  const { layout } = useServerAreas();
  return (
    <div
      className="workspace-frame workspace-layout-row"
      ref={layout.attachRow}
    >
      {children}
    </div>
  );
});
