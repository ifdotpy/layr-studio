import { useMemo } from "react";
import { reaction, runInAction } from "mobx";
import { useEffect, useState, type ReactNode } from "react";
import { AgentStatus } from "../../../generated/api.ts";
import {
  TeamProvider,
  useTeamArea,
} from "../../agents/support/TeamPanelContext.tsx";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const AppTeam = observer(function AppTeam({
  children,
}: {
  children: ReactNode;
}) {
  const a = useServerAreas(),
    ui = a.ui,
    lead = a.navigation.selectedLead;
  const [query, setQuery] = useState(""),
    [filter, setFilter] = useState("all"),
    [override, setOverride] = useState<{
      id: string | null;
      value: boolean;
    } | null>(null);
  const view = useTeamArea(
    a.scope.screen.active ? lead?.id : undefined,
    query,
    filter,
    a.read.pendingReadTargets,
    a.selected.hasRetainedTeam,
  );
  useEffect(
    () =>
      reaction(
        () => a.navigation.selectedId,
        () => setOverride(null),
      ),
    [a],
  );
  useEffect(() => {
    setQuery("");
    setFilter("all");
  }, [lead?.id]);
  const actions = useMemo(
    () => ({
      setCompletedOpen: (value: boolean) =>
        setOverride({ id: a.navigation.selectedId, value }),
      setTeamOpen: (open: boolean) =>
        runInAction(() => {
          ui.teamOpen = open;
        }),
      setWideTeamOpen: (open: boolean) => ui.setTeamPreferred(open),
      open: a.open,
      previewResult: ui.showPreview,
      remove: (id: string) => ui.requestDelete({ id }, false, true),
    }),
    [a, ui],
  );
  if (view.view.members.length && ui.isTeamVisible)
    a.scope.services.tokenRates.observeTokenRates();
  const value = useMemo(
    () => ({
      view: view.view,
      retainedWorktrees: view.retainedWorktrees,
      hasTeam: () => view.view.workers.length > 0 || a.selected.hasRetainedTeam,
      get opened() {
        return a.navigation.selectedId;
      },
      workerQuery: query,
      setWorkerQuery: setQuery,
      workerFilter: filter,
      setWorkerFilter: setFilter,
      get completedOpen() {
        const selected = a.navigation.selectedAgent;
        return override?.id === a.navigation.selectedId
          ? override.value
          : !!selected &&
              !selected.isLead &&
              selected.status === AgentStatus.completed;
      },
      ...actions,
    }),
    [a, view.view, query, filter, override, actions],
  );
  return <TeamProvider value={value}>{children}</TeamProvider>;
});
