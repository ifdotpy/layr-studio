import type { SidebarChatRow } from "../../generated/observableResources.ts";
import type { PeerTeam } from "../../types.ts";

/** Index the server's ordered rows without reconstructing visibility or order. */
export function sidebarRows(
  rows: readonly SidebarChatRow[],
  teams: PeerTeam[],
) {
  const byId = new Map(rows.map((row) => [row.id, row]));
  const teamsById = new Map(teams.map((team) => [team.id, team]));
  const byTeam = new Map<string, PeerTeam>();
  const byProject = new Map<string, SidebarChatRow[]>();
  const grouped = new Set<string>();
  for (const row of rows) {
    const project = byProject.get(row.projectPath);
    if (project) project.push(row);
    else byProject.set(row.projectPath, [row]);
    const team = row.teamId === null ? undefined : teamsById.get(row.teamId);
    if (team) {
      byTeam.set(row.id, team);
      if (team.projectPath === row.projectPath) grouped.add(row.id);
    }
  }
  return { agents: rows, ordered: rows, byId, byProject, byTeam, grouped };
}
