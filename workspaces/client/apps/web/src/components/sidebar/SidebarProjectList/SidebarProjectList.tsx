import { observer } from "mobx-react-lite";

import { sidebarRender } from "../../../perf/sidebarRenderProbe.ts";
import { SidebarProjectSection } from "../SidebarProjectSection/SidebarProjectSection.tsx";
import { useSidebarState } from "../support/SidebarState.tsx";
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
