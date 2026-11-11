import type { SidebarBackend } from "../components/sidebar/support/services.tsx";
import type { ServerCommand } from "./navigation.ts";

export interface CombinedSidebarServices {
  backends: ReadonlyMap<string, SidebarBackend>;
  online(owner: string): boolean;
  executeCommand(owner: string, command: ServerCommand): Promise<void>;
  aliases: Record<string, string>;
  send(owner: string, command: ServerCommand): void;
  settings(): void;
  search(): void;
  notify(message: string): void;
}
