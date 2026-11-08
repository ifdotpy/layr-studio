import { useTeam } from "../support/TeamPanelContext.tsx";
import { observer } from "mobx-react-lite";
import type { ReactNode } from "react";

export const TeamPresence = observer(function TeamPresence({
  children,
}: {
  children: ReactNode;
}) {
  return useTeam().hasTeam() ? children : null;
});
