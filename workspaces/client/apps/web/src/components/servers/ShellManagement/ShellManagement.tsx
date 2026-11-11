import { observer } from "mobx-react-lite";
import ServerManager from "../ServerManager/ServerManager.tsx";
import { ShellState } from "../../../app/shell/ShellState.ts";

export const ShellManagement = observer(function ShellManagement({
  shell,
}: {
  shell: ShellState;
}) {
  const discovery = shell.discovery;
  return (
    <ServerManager
      rows={shell.serverRows}
      servers={shell.servers}
      add={shell.add}
      remove={shell.remove}
      discovery={discovery}
      localEnabled={discovery.enabled}
    />
  );
});
