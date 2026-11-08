import type { ReactNode } from "react";

export function SettingsRow({
  label,
  help,
  children,
  stacked,
  className,
}: {
  label: ReactNode;
  help?: ReactNode;
  children: ReactNode;
  /** Put the label above a wide control, such as account tiles. */
  stacked?: boolean;
  className?: string;
}) {
  return (
    <div
      className={`ui-settings-row${stacked ? " ui-settings-row-stacked" : ""}${className ? ` ${className}` : ""}`}
    >
      <div className="ui-label">
        <div className="ui-row-label">{label}</div>
        {help && <div className="ui-help">{help}</div>}
      </div>
      <div className="ui-row-control">{children}</div>
    </div>
  );
}
