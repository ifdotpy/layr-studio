import { Button } from "@mantine/core";
import type { AgentModel } from "../../../app/contracts.ts";

type FolderProps = {
  agent: Pick<AgentModel, "projectPath" | "legacyVmWorkspace">;
  /** Only the desktop app on its own server can reveal the folder. */
  canReveal: boolean;
  onReveal: (path: string) => void;
};

/** Show the project folder when it is available on the server. */
export default function ProjectFolderDialog(props: FolderProps) {
  const { agent, canReveal, onReveal } = props;
  if (agent.legacyVmWorkspace) return <p>This chat’s folder is unavailable.</p>;
  return (
    <>
      <p>{agent.projectPath}</p>
      <p>To use another folder, start a new chat.</p>
      {canReveal && agent.projectPath && (
        <Button onClick={() => onReveal(agent.projectPath || "")}>
          Show folder
        </Button>
      )}
    </>
  );
}
