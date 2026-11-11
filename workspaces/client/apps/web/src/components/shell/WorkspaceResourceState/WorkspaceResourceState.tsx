import { Loader } from "@mantine/core";
import { observer } from "mobx-react-lite";

export const WorkspaceResourceState = observer(function WorkspaceResourceState({
  state,
}: {
  state: { error: string; loading: boolean; data: unknown };
}) {
  return state.error ? (
    <p role="alert" className="workspace-error">
      {state.error}
    </p>
  ) : state.loading && !state.data ? (
    <div className="workspace-load">
      <Loader size="sm" />
    </div>
  ) : null;
});
