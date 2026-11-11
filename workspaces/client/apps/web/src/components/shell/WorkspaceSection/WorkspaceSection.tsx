import type { ComponentType } from "react";
import { localDateTime } from "../../../local-time.ts";
import { EmptyState } from "../../ui/EmptyState/EmptyState.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import type { Context } from "../Workspace/Workspace.tsx";
import { WorkspacePanel } from "../../../app/shell/WorkspaceStore.ts";

export const date = (value: number | string | undefined) =>
  value
    ? localDateTime(new Date(typeof value === "number" ? value * 1000 : value))
    : "";

export function WorkspaceSection({ children }: { children: React.ReactNode }) {
  return <EmptyState title={children} />;
}

export type PanelContext<T = never> = Context & { panel: WorkspacePanel<T> };

export function section<T>(View: ComponentType<PanelContext<T>>) {
  return function Section(context: Context) {
    return (
      <Owned create={() => new WorkspacePanel<T>()}>
        {(panel) => <View {...context} panel={panel} />}
      </Owned>
    );
  };
}
