import { ServerAreas, Provider } from "../../../app/ServerAreasProvider.tsx";
import { useEffect, type ReactNode } from "react";

export function ServerAreasProvider({
  areas,
  children,
}: {
  areas: ServerAreas;
  children: ReactNode;
}) {
  useEffect(() => areas.start(), [areas]);
  return <Provider value={areas}>{children}</Provider>;
}
