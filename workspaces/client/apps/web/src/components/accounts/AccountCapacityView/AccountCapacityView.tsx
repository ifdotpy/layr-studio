import type { Account } from "../support/Accounts.tsx";
import { AccountStatus as GeneratedApiAccountStatus } from "../../../generated/api.ts";
import { remainingText } from "../support/AccountTiles.tsx";
import type { AccountLimitView } from "../AccountTiles/AccountTiles.tsx";
import { localDateTime } from "../../../local-time.ts";
import ErrorDescription from "../../shell/ErrorDescription/ErrorDescription.tsx";
import { Accordion, Progress } from "@mantine/core";
import type { ReactNode } from "react";

import { remainingLimitColor } from "../usage/support/accountLimitColor.ts";

/** One reading of an account's provider limits. `note` says where and when
 * the reading came from when the account has more than one source. */
export function AccountCapacityView({
  account,
  view,
  card = false,
  note,
  extraDetails,
  facts,
}: {
  account: Pick<Account, "status" | "email" | "label">;
  view: AccountLimitView;
  card?: boolean;
  note?: ReactNode | undefined;
  extraDetails?: ReactNode | undefined;
  facts?: ReactNode | undefined;
}) {
  const { limits, buckets, remaining, color, reset, error: limitsError } = view;
  const availableCredits = reset?.credits ?? [];
  const windows = (
    <div className="account-window-grid">
      <table className="account-limits-table">
        <thead>
          <tr>
            <th>Window</th>
            <th>Meter</th>
            <th>Left</th>
            <th>Resets at</th>
          </tr>
        </thead>
        <tbody>
          {buckets.flatMap((bucket) =>
            bucket.windows.map((window, index) => (
              <tr key={`${bucket.id}-${index}`}>
                <th scope="row">
                  <span>{bucket.shortName}</span>
                  <small>{window.label}</small>
                </th>
                <td>
                  {window.remaining !== null && !window.expired ? (
                    <Progress.Root
                      className="account-window-meter"
                      size="xs"
                      role="meter"
                      aria-label={`${bucket.name} ${window.label} remaining`}
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-valuenow={window.remaining}
                    >
                      <Progress.Section
                        value={window.remaining}
                        color={remainingLimitColor(window.color)}
                        withAria={false}
                      />
                    </Progress.Root>
                  ) : (
                    <span>—</span>
                  )}
                </td>
                <td>
                  {window.expired
                    ? "Reset due"
                    : window.remaining === null
                      ? "Unknown"
                      : remainingText(window.remaining)}
                </td>
                <td>
                  {window.reset ? (
                    <time
                      dateTime={new Date(window.reset * 1000).toISOString()}
                      title={localDateTime(new Date(window.reset * 1000))}
                    >
                      {localDateTime(new Date(window.reset * 1000), {
                        month: "short",
                        day: "numeric",
                        hour: "2-digit",
                        minute: "2-digit",
                      })}
                    </time>
                  ) : (
                    "Unavailable"
                  )}
                </td>
              </tr>
            )),
          )}
          {!buckets.length && (
            <tr>
              <td colSpan={4}>
                {limitsError
                  ? "Limits unavailable"
                  : view.reading
                    ? "Reading limits…"
                    : "Limits unavailable"}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
  const errors = limitsError && (
    <ErrorDescription
      className="account-action-error"
      role="status"
      value={limits?.readError || limitsError}
    />
  );
  if (!card)
    return (
      <div className="account-capacity">
        {errors}
        {windows}
      </div>
    );
  return (
    <div
      className="account-capacity"
      aria-label={`Limits for ${account.email || account.label}`}
    >
      <div className="account-limit-summary">
        <span>Remaining limit</span>
        <span
          role={remaining === null ? undefined : "meter"}
          aria-label="Remaining limit"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={remaining ?? undefined}
        >
          <i
            className="account-summary-dot"
            aria-hidden="true"
            style={{ background: remainingLimitColor(color) }}
          />
          {remaining === null
            ? view.reading
              ? "Reading limits…"
              : "Unavailable"
            : remainingText(remaining)}
        </span>
      </div>
      <Accordion.Panel className="account-expanded">
        <section className="account-detail-block">
          <h4>Limits</h4>
          {note && <small className="account-limit-note">{note}</small>}
          {errors}
          {windows}
        </section>
        <section className="account-detail-block">
          <h4>Resets</h4>
          <p>
            {reset?.present ? reset.countLabel : "No reset credits reported."}
          </p>
          {availableCredits.map(
            (credit) =>
              credit.expiresAt !== null && (
                <p key={credit.id}>
                  Expires{" "}
                  <time
                    dateTime={new Date(credit.expiresAt * 1000).toISOString()}
                  >
                    {localDateTime(new Date(credit.expiresAt * 1000))}
                  </time>
                </p>
              ),
          )}
          <small>Redeem reset credits from Limits in a chat.</small>
        </section>
        <section className="account-detail-block">
          <h4>Account</h4>
          {facts || (
            <dl className="account-facts">
              <dt>Email</dt>
              <dd>{account.email || "Unavailable"}</dd>
              <dt>Sign-in</dt>
              <dd>
                {account.status === GeneratedApiAccountStatus.ready
                  ? "Signed in"
                  : "Sign in needed"}
              </dd>
            </dl>
          )}
        </section>
        {extraDetails && (
          <section className="account-detail-block">
            <h4>Actions</h4>
            {extraDetails}
          </section>
        )}
      </Accordion.Panel>
    </div>
  );
}
