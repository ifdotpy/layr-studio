import { observer } from "mobx-react-lite";
import { ActionIcon, Menu } from "@mantine/core";
import { MoreHorizontal, Trash2 } from "lucide-react";
import { useTeam } from "../TeamPanelContext.tsx";
import type { WorkerProps } from "../WorkerCard.tsx";

export const WorkerOptions = observer(function WorkerOptions({
  agent,
}: WorkerProps) {
  const { remove, setTeamOpen } = useTeam();
  return (
    <Menu withinPortal position="bottom-end">
      <Menu.Target>
        <ActionIcon
          className="worker-options"
          variant="subtle"
          color="gray"
          size="sm"
          aria-label={`Options for subagent ${agent.name}`}
        >
          <MoreHorizontal size={16} />
        </ActionIcon>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Item
          color="red"
          leftSection={<Trash2 size={14} />}
          onClick={() => {
            setTeamOpen(false);
            remove(agent.id);
          }}
        >
          Delete
        </Menu.Item>
      </Menu.Dropdown>
    </Menu>
  );
});
