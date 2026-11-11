import type { ReactNode } from "react";
import { TeamProvider } from "../../agents/support/TeamPanelContext.tsx";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export function AppTeam({ children }: { children: ReactNode }) {
  return <TeamProvider value={useServerAreas().team}>{children}</TeamProvider>;
}
