import { ServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { ServerAreasContextProvider as Provider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
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
