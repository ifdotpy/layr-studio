import { ServerAreas, Provider } from "../../../app/ServerAreasProvider.tsx";
import { type ReactNode } from "react";

export function ServerAreasProvider({
  areas,
  children,
}: {
  areas: ServerAreas;
  children: ReactNode;
}) {
  return <Provider value={areas}>{children}</Provider>;
}
