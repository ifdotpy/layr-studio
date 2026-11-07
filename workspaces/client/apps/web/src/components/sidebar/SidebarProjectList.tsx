import { observer } from "mobx-react-lite";

import { sidebarRender } from "./renderProbe.ts";
import { SidebarProjectSection } from "./SidebarProjectSection.tsx";
import { useSidebarState } from "./SidebarState.tsx";
export const SidebarProjectList = observer(function SidebarProjectList() {
  const sidebar = useSidebarState();
  sidebarRender("SidebarProjectList");
  return (
    <>
      {sidebar.projectGroups.map((group) => (
        <SidebarProjectSection key={group.path} group={group} />
      ))}
    </>
  );
});
