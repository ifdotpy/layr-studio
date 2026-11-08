import { createArea } from "../../../app/area.ts";
import type { SidebarAgent } from "./viewModel.ts";
export interface SidebarCommands {
  settings?: () => void;
  open: (id: string) => void;
  newChat: (path?: string, folder?: string) => void;
  newSharedChat?: (path?: string) => void;
  addProject: () => void;
  changeProject: (id: string) => void;
  projectAccount: (path: string) => void;
  projectFolders?: (path: string) => void;
  deleteProject?: (path: string) => void;
  rename: (id: string, name: string) => Promise<void>;
  remove: (id: string, room: boolean) => void;
  onSearch: () => void;
  refresh?: () => Promise<void>;
  notify?: (message: string) => void;
  markUnread: (
    id: string,
    target: NonNullable<SidebarAgent["markRead"]>,
  ) => void;
}
export const [SidebarCommandsProvider, useSidebarCommands] =
  createArea<SidebarCommands>("Sidebar");
