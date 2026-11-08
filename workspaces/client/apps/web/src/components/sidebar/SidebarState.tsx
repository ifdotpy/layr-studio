import { createArea } from "../area.ts";
import type { SidebarListOwner } from "../../app/shell/SidebarOwner.ts";
export type { NavigableProject } from "../../app/shell/SidebarOwner.ts";
export const [SidebarStateProvider, useSidebarState] =
  createArea<SidebarListOwner>("Sidebar state");
