import { createArea } from "../components/area.ts";
import type { SidebarBackend } from "../components/sidebar/services.tsx";
import type { ServerCommand, ServerNavigation } from "./navigation.ts";

export interface CombinedSidebarServices {
  backends: ReadonlyMap<string, SidebarBackend>;
  online(owner: string): boolean;
  refreshOwner(owner: string): Promise<ServerNavigation>;
  executeCommand(owner: string, command: ServerCommand): Promise<void>;
  aliases: Record<string, string>;
  send(owner: string, command: ServerCommand): void;
  applyNavigation(owner: string, navigation: ServerNavigation): void;
  settings(): void;
  search(): void;
  notify(message: string): void;
}
export const [CombinedSidebarServicesProvider, useCombinedSidebarServices] =
  createArea<CombinedSidebarServices>("Combined sidebar");
