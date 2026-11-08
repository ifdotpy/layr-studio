import { observer } from "mobx-react-lite";
import type { ReactNode } from "react";
import { EdgeRail } from "../edge-rail/EdgeRail/EdgeRail.tsx";
import { useServerAreas as useNavigationButtons } from "../../../app/ServerAreasProvider.tsx";

export const RailRoot = observer(function RailRoot({
  menu,
}: {
  menu: ReactNode;
}) {
  const { layout, sidebar, navigation, ui } = useNavigationButtons();
  const workspace = ui;
  return (
    <EdgeRail
      geometry={layout.railGeometry}
      sidebar={{
        id: "sidebar-toggle",
        label: sidebar.toggleLabel,
        expanded: sidebar.isExpanded,
        onClick: sidebar.toggle,
      }}
      messages={{
        count: workspace.attentionCount,
        disabled: !navigation.selectedLead,
        onClick: () => workspace.showMessages(),
      }}
      menu={menu}
    />
  );
});
