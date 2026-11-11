import { Accordion, Badge, Menu } from "@mantine/core";
import { Check, CloudOff, Ellipsis, LogIn } from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import { AccountStatus } from "../../../generated/api.ts";
import {
  PROVIDER_LABELS,
  ProviderKind,
} from "../../../generated/providerKind.ts";
import type { ServerAccountRow } from "../../../servers/serverAccountRows.ts";
import { AccountCapacityView } from "../../accounts/AccountCapacityView/AccountCapacityView.tsx";
import { ProviderMark } from "../../accounts/ProviderMark/ProviderMark.tsx";
import { accountLimitView } from "../../accounts/support/AccountTiles.tsx";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import { AccountLimitsAcrossServers } from "../AccountLimitsAcrossServers/AccountLimitsAcrossServers.tsx";
import { ServerAccountGroup } from "../ServerAccountGroup/ServerAccountGroup.tsx";
import type { AccountsPanelState } from "../../../servers/AccountsPanelState.ts";

type RowView = import("../../../servers/AccountsPanelState.ts").AccountRowView;
export const AccountRow = observer(function AccountRow({
  view,
  ui,
  focus,
}: {
  view: RowView;
  ui: AccountsPanelState;
  focus: string | undefined;
}) {
  const { row } = view;
  const facts = (
    <>
      <dl className="account-facts">
        {Object.entries(view.facts).map(([label, value]) => (
          <Fragment key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </Fragment>
        ))}
      </dl>
      <small className="server-accounts-help">
        Limits are shared across servers. Sign-in and turn start limits are set
        per server.
      </small>
    </>
  );
  const details = (
    <ul
      className="server-account-servers"
      aria-label={`Servers for ${row.email || row.label}`}
    >
      {view.chips.map((item) => (
        <li
          key={item.server.id}
          className="server-account-server"
          data-server-id={item.server.id}
        >
          <button
            type="button"
            className="server-account-chip"
            data-state={item.state}
            disabled={item.blocked}
            onClick={item.signIn}
          >
            {item.chip}
          </button>
          {item.source && (
            <ServerAccountGroup source={item.source} onSaved={() => {}} />
          )}
        </li>
      ))}
    </ul>
  );
  return (
    <Accordion.Item
      value={row.key}
      className="server-account-row"
      data-account-identity={row.key}
    >
      <div className="server-account-heading">
        <Accordion.Control
          aria-label={`Limit details for ${row.email || row.label}`}
        >
          <span className="account-identity">
            <span role="img" aria-label={PROVIDER_LABELS[row.provider]}>
              <ProviderMark provider={row.provider} />
            </span>
            <strong title={row.label || row.email || "Account"}>
              {row.label || row.email || "Account"}
            </strong>
          </span>
        </Accordion.Control>
        {row.plan && <span className="account-plan">{row.plan}</span>}
        <AccountMarkers markers={view.markers} />
        <AccountMenu row={row} view={view} ui={ui} focus={focus === row.key} />
        <div
          className="server-account-login-actions"
          role="group"
          aria-label={`Sign in for ${row.email || row.label}`}
        >
          {view.present
            .filter((item) => item.needsSignIn)
            .map((item) => (
              <ActionButton
                key={item.server.id}
                actionRole="secondary"
                leftSection={<LogIn size={14} aria-hidden="true" />}
                aria-label={`Sign in to ${row.label || row.email} on ${item.server.label}`}
                disabled={item.blocked}
                onClick={item.signIn}
              >
                Sign in on {item.server.label}
              </ActionButton>
            ))}
          {view.present
            .filter((item) => !item.account?.accountKey)
            .map((item) => (
              <span role="status" key={item.server.id}>
                {item.server.label}: Account identity unknown.
              </span>
            ))}
        </div>
      </div>
      {view.sources.length ? (
        <AccountLimitsAcrossServers
          sources={view.sources}
          extraDetails={details}
          facts={facts}
        />
      ) : (
        <AccountCapacityView
          card
          account={{
            status: AccountStatus.signedOut,
            email: row.email,
            label: row.label,
          }}
          view={accountLimitView(
            { id: row.key, status: AccountStatus.signedOut },
            null,
          )}
          facts={facts}
          extraDetails={details}
        />
      )}
    </Accordion.Item>
  );
});

function AccountMarkers({ markers }: Pick<RowView, "markers">) {
  const icons = { connection: CloudOff, signIn: LogIn, default: Check };
  return (
    <span className="account-status-markers">
      {markers.map((marker, index) => {
        const Icon = icons[marker.kind];
        const content = (
          <>
            <Icon
              size={marker.kind === "default" ? 12 : 14}
              aria-hidden="true"
            />
            <span
              className={
                marker.kind === "default"
                  ? "account-strip-state-label"
                  : undefined
              }
            >
              {marker.kind === "signIn" ? (
                <>
                  {marker.prefix}
                  <span>{marker.detail}</span>
                </>
              ) : (
                marker.label
              )}
            </span>
          </>
        );
        return marker.kind === "default" ? (
          <Badge key={index} size="xs" variant="default" title={marker.label}>
            {content}
          </Badge>
        ) : (
          <span
            key={index}
            className="account-strip-state"
            title={marker.label}
          >
            {content}
          </span>
        );
      })}
    </span>
  );
}

const operations = [
  ["configure-profile", "Configure"],
  ["rename", "Rename"],
  ["default", "Make default"],
  ["disconnect", "Sign out"],
  ["remove", "Remove"],
] as const;
const AccountMenu = observer(function AccountMenu({
  row,
  view,
  ui,
  focus,
}: {
  row: ServerAccountRow;
  view: RowView;
  ui: AccountsPanelState;
  focus: boolean;
}) {
  const opened = ui.escape.openedKey === row.key;
  const escape = ui.escape;
  return (
    <Menu
      opened={opened}
      onChange={(next) => escape.changeMenu(row.key, next)}
      position="bottom-end"
      withinPortal={false}
      withInitialFocusPlaceholder={false}
    >
      <Menu.Target>
        <ActionButton
          className="account-card-menu"
          data-autofocus={focus || undefined}
          {...escape.triggerProps(row.key)}
          actionRole="quiet"
          aria-label={`Actions for ${row.email || row.label}`}
        >
          <Ellipsis size={18} />
        </ActionButton>
      </Menu.Target>
      <Menu.Dropdown data-mantine-stop-propagation="true">
        {view.present.map((item) => (
          <div key={item.server.id}>
            <Menu.Label>{item.server.label}</Menu.Label>
            {operations
              .filter(
                ([operation]) =>
                  row.provider === ProviderKind.Claude ||
                  operation !== "configure-profile",
              )
              .map(([operation, label]) => (
                <Menu.Item
                  key={operation}
                  disabled={item.blocked}
                  {...(operation === "remove" || operation === "disconnect"
                    ? { color: "red" }
                    : {})}
                  onClick={() => item.act(operation)}
                >
                  {label}
                </Menu.Item>
              ))}
            <Menu.Item
              disabled={item.blocked}
              onClick={item.signIn}
            >{`Sign in again on ${item.server.label}`}</Menu.Item>
          </div>
        ))}
      </Menu.Dropdown>
    </Menu>
  );
});
