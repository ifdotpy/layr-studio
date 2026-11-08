import type { ReactNode } from "react";

type HeaderProps = {
  title: ReactNode;
  actions?: ReactNode | undefined;
  help?: ReactNode | undefined;
};

export function PanelHeader({ title, actions, help }: HeaderProps) {
  return (
    <header className="ui-panel-header">
      <div className="ui-label">
        <h3>{title}</h3>
        {help && <div className="ui-help">{help}</div>}
      </div>
      {actions && <div className="ui-actions">{actions}</div>}
    </header>
  );
}

export function SettingsSection({
  title,
  help,
  action,
  children,
}: Omit<HeaderProps, "actions"> & { action?: ReactNode; children: ReactNode }) {
  return (
    <section className="ui-settings-section">
      <PanelHeader title={title} help={help} actions={action} />
      <div className="ui-section-body">{children}</div>
    </section>
  );
}
