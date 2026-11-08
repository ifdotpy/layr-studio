import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export function WorkspaceSlot() {
  const { layout } = useServerAreas();
  return (
    <div
      id="workspace-panel-slot"
      className="workspace-panel-slot"
      ref={layout.attachSlot}
    />
  );
}
