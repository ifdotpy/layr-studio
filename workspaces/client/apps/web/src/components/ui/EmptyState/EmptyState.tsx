import type { ReactNode } from "react";

export function EmptyState({
  title,
  help,
  action,
}: {
  title: ReactNode;
  help?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="ui-empty-state">
      <div className="ui-label">
        <div className="ui-row-label">{title}</div>
        {help && <div className="ui-help">{help}</div>}
      </div>
      {action && <div className="ui-actions">{action}</div>}
    </div>
  );
}
