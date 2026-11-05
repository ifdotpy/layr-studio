import { Button } from "@mantine/core";
import type { Agent } from "../types";

type FolderProps = {
  agent: Agent;
  /** Only the desktop app on its own server can reveal a Mac path. */
  canReveal: boolean;
  onReveal: (path: string) => void;
};

export default function ProjectFolderDialog(props: FolderProps) {
  const { agent, canReveal, onReveal } = props;
  return (
    <>
      <p>{agent.projectPath}</p>
      <p>To use another folder, start a new chat.</p>
      {canReveal && agent.projectPath && (
        <Button onClick={() => onReveal(agent.projectPath!)}>
          Show folder
        </Button>
      )}
    </>
  );
}
