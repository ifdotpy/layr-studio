import type { ComponentProps } from "react";
import ProjectAccount from "../ProjectAccount/ProjectAccount.tsx";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const ProjectAccountSettings = observer(function ProjectAccountSettings({
  path,
  saves,
}: {
  path: string;
  saves: ComponentProps<typeof ProjectAccount>["saves"];
}) {
  const a = useServerAreas();
  const accounts = a.accounts;
  if (!accounts) return <p role="status">Loading accounts…</p>;
  const projects = [...(a.state?.projects.values() || [])];
  return (
    <ProjectAccount
      path={path}
      project={projects.find((p) => p.path === path)}
      accounts={accounts}
      defaultAccountKey={
        projects
          .filter(
            (p) =>
              p.path &&
              (path === p.path ||
                path.startsWith(p.path.replace(/\/$/, "") + "/")),
          )
          .sort((x, y) => (y.path?.length || 0) - (x.path?.length || 0))[0]
          ?.accountKey || accounts.defaultAccountKey
      }
      saves={saves}
    />
  );
});
