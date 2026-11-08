import { observer } from "mobx-react-lite";

import { sidebarRender } from "../../../perf/sidebarRenderProbe.ts";
import {
  useSidebarState,
  type NavigableProject,
} from "../support/SidebarState.tsx";

export const CompactChatCount = observer(function CompactChatCount({
  group,
}: {
  group: NavigableProject;
}) {
  const sidebar = useSidebarState();
  sidebarRender("CompactChatCount");
  return (
    <>
      Show old (
      {Math.max(
        0,
        sidebar.model.visibleRows.filter(
          (row) => row.projectPath === group.path,
        ).length - group.chats.length,
      )}
      )
    </>
  );
});
