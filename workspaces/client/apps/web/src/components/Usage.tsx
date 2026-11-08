import { observer } from "mobx-react-lite";

import { LimitColor, RunwayColor } from "../generated/api";
import { PassiveUsage } from "../hooks/BottomInsetBand";
import { ProviderKind, providerLabel } from "../generated/providerKind.ts";
import { localDateTime, localTime } from "../local-time";
import { lazy, Suspense, useState, useMemo, useEffect, useRef } from "react";
import { Button, Popover, Progress, Tabs, Tooltip } from "@mantine/core";
import BottomSheet from "./ui/BottomSheet";
import { useMediaQuery } from "@mantine/hooks";
import { layoutQueries } from "../layout/layout-contract.generated.ts";
import {
  useServerApi,
  useServerStore,
} from "../servers/ServerServicesContext.tsx";
import { resourceData } from "../store/runtime.ts";
import { agentId } from "../generated/observableResources.ts";
import { accountLimits, jsonObject } from "../usage/accountUsage.ts";
import { useLimitRefresh } from "../usage/useLimitRefresh.ts";
import { accountsForRoles } from "./roleAccounts.ts";
import { errorText } from "../api.ts";
import { randomUUID } from "../uuid.ts";
import type { Agent } from "../types.ts";
import {
  ChevronUp,
  ExternalLink,
  Gauge,
  RefreshCw,
  CircleDollarSign,
} from "lucide-react";
import LimitRecoveryNotice from "./LimitRecoveryNotice";
import "./Usage.css";
import { remainingLimitColor } from "./accountLimitColor";
import TokenRate from "./TokenRate";
import {
  AccountTilesLayout,
  AccountRemainingBar,
  AccountTileLabel,
} from "./AccountTiles";
import { accountDisplayName } from "../accountName";
const Analytics = lazy(() => import("./Analytics"));

import type { Account } from "./Accounts";
type UsageIdentity = Pick<Account, "id" | "label" | "email" | "provider">;
const humanAccountLabel = (account?: UsageIdentity) =>
  account?.email ||
  (account?.label && account.label !== account.id ? account.label : "") ||
  "Account name unavailable";

const displayAccountName = (account?: UsageIdentity) =>
  humanAccountLabel(account) === "Account name unavailable"
    ? "Account name unavailable"
    : accountDisplayName({
        label: account?.label === account?.id ? "" : account?.label || "",
        email: account?.email,
      });
const accountProvider = (account?: UsageIdentity) => account?.provider || "";

const number = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);
export const formatPercent = (value: number) =>
  value > 0 && value < 1 ? "<1%" : `${Math.floor(value)}%`;
const dollarFormatter = new Intl.NumberFormat(undefined, {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 2,
});
const dollars = (value: unknown) =>
  number(value) ? dollarFormatter.format(value) : "Unavailable";
export type UsageProps = {
  agent: Agent;
  opened?: boolean;
  onChange?: (opened: boolean) => void;
  openRequest?: number;
  visible?: boolean;
};
function Usage({
  agent,
  opened: controlled,
  onChange,
  openRequest = 0,
  visible = true,
}: UsageProps) {
  const store = useServerStore(),
    api = useServerApi();
  const [localOpen, setLocalOpen] = useState(controlled ?? false),
    [analyticsOpen, setAnalyticsOpen] = useState(false),
    [contextOpen, setContextOpen] = useState(false);
  const opened = onChange ? (controlled ?? false) : localOpen;
  const changeOpened = onChange ?? setLocalOpen;
  const [browse, setBrowse] = useState({
    key: null as string | null,
    request: openRequest,
  });
  const [resetPending, setResetPending] = useState(false),
    [resetApplied, setResetApplied] = useState(new Set<string>());
  const [feedback, setFeedback] = useState({
    key: "",
    confirm: null as string | null,
    error: "",
    notice: "",
  });
  const [costInterestError, setCostInterestError] = useState("");
  const lifetime = useMemo(
    () => ({
      active: true,
      resetBusy: false,
      resetAttempts: new Map<string, string>(),
    }),
    [api],
  );
  useEffect(() => {
    lifetime.active = true;
    return () => {
      lifetime.active = false;
    };
  }, [lifetime]);
  const { reload, isRefreshing } = useLimitRefresh(api.get);
  const detail = resourceData(store.agentDetail(agentId(agent.id)).current);
  const fallbackKey = agent.accountKey || "default";
  const visibleUsageAccounts =
    resourceData(store.state().current)?.accountsSummary.get("current")
      ?.accounts ?? [];
  const wantedKey = browse.request === openRequest ? browse.key : null;
  const selectedAccount =
    visibleUsageAccounts.find((a) => a.id === (wantedKey || fallbackKey)) ||
    visibleUsageAccounts[0];
  const selectedAccountKey = selectedAccount?.id || fallbackKey;
  const projectionFor = (account: Account) =>
    resourceData(store.state().current)?.rateLimitsByAccount.get(account.id);
  const limitsFor = (account: Account) =>
    accountLimits(
      projectionFor(account)?.observation,
      account.id,
      account.accountId,
    );
  const activeAccountLimits = selectedAccount
    ? limitsFor(selectedAccount)
    : null;
  const context = jsonObject(detail?.contextUsage),
    contextTokens = context?.tokens,
    contextWindow = context?.window;
  const contextProgress =
    typeof contextTokens === "number" &&
    Number.isFinite(contextTokens) &&
    typeof contextWindow === "number" &&
    Number.isFinite(contextWindow) &&
    contextWindow > 0
      ? {
          tokens: contextTokens,
          window: contextWindow,
          percent: Math.round((contextTokens / contextWindow) * 100),
        }
      : null;
  const lowAccount = visibleUsageAccounts.find(
    (a) => projectionFor(a)?.projection.buckets?.current.low,
  );
  const roleAccounts = accountsForRoles(
    agent,
    visibleUsageAccounts,
    detail || {},
    (a) => a.id,
    (a) => projectionFor(a)?.projection.runway,
  );
  const sessionCost = visible
    ? resourceData(store.sessionCost(agentId(agent.rootId || agent.id)).current)
    : undefined;
  const rows =
    visible && opened ? resourceData(store.costs().current) : undefined;
  const costs = rows?.find((row) => row.accountKey === selectedAccountKey);
  const key = visible && opened ? selectedAccountKey : null,
    ready = rows !== undefined,
    signature = costs ? JSON.stringify(costs) : null;
  const interest = useRef<{
    key: string;
    baseline: string | null | undefined;
    phase: "baseline" | "pending" | "settled";
  } | null>(null);
  useEffect(() => {
    if (!key) {
      interest.current = null;
      setCostInterestError("");
      return;
    }
    if (interest.current?.key !== key) {
      interest.current = { key, baseline: undefined, phase: "baseline" };
      setCostInterestError("");
    }
    const attempt = interest.current;
    if (attempt.phase === "pending" && ready && attempt.baseline === undefined)
      attempt.baseline = signature;
    if (
      attempt.phase === "pending" &&
      ready &&
      signature !== attempt.baseline
    ) {
      attempt.phase = "settled";
      setCostInterestError("");
    }
    if (attempt.phase !== "baseline") return;
    attempt.baseline = ready ? signature : undefined;
    attempt.phase = "pending";
    void api
      .post("/api/costs/interest", { account_key: key })
      .catch((error) => {
        if (
          lifetime.active &&
          interest.current === attempt &&
          attempt.phase === "pending"
        )
          setCostInterestError(errorText(error));
      });
  }, [key, ready, signature, api, lifetime]);
  const currentAccount = useRef(selectedAccountKey);
  currentAccount.current = selectedAccountKey;
  const resetAppliedKey = (credit: string, account = selectedAccountKey) =>
    account + ":" + credit;
  const selectAccount = (key: string | null) => {
    if (!key || key === selectedAccountKey) return;
    setBrowse({ key, request: openRequest });
    setFeedback({ key, confirm: null, error: "", notice: "" });
  };
  const requestReset = (credit: string | null) =>
    setFeedback({
      key: selectedAccountKey,
      confirm: credit,
      error: "",
      notice: "",
    });
  const applyReset = async (creditId: string) => {
    const account = selectedAccount;
    if (
      lifetime.resetBusy ||
      !account ||
      resetApplied.has(resetAppliedKey(creditId, account.id))
    )
      return;
    const accountKey = account.id;
    const feedbackCurrent = () =>
      lifetime.active && currentAccount.current === accountKey;
    lifetime.resetBusy = true;
    setResetPending(true);
    setFeedback({ key: accountKey, confirm: creditId, error: "", notice: "" });
    try {
      const accountId = projectionFor(account)?.projection.reset.accountId;
      if (!accountId)
        throw new Error("Refresh limits before using this reset credit.");
      const key = accountId + ":" + creditId;
      const requestId = lifetime.resetAttempts.get(key) || randomUUID();
      lifetime.resetAttempts.set(key, requestId);
      const result = await api.post("/api/limits/reset", {
        credit_id: creditId,
        account_id: accountId,
        account_key: accountKey,
        request_id: requestId,
      });
      if (result.outcome !== "uncertain") lifetime.resetAttempts.delete(key);
      if (
        lifetime.active &&
        (result.outcome === "reset" || result.outcome === "alreadyRedeemed")
      )
        setResetApplied((ids) =>
          new Set(ids).add(resetAppliedKey(creditId, accountKey)),
        );
      if (feedbackCurrent()) {
        const notice = {
          reset: "Reset applied.",
          alreadyRedeemed: "This reset credit was already used.",
          nothingToReset: "No limits need a reset.",
          noCredit: null,
          uncertain: null,
        }[result.outcome];
        setFeedback({
          key: accountKey,
          confirm: notice ? null : creditId,
          notice: notice || "",
          error: notice
            ? ""
            : result.outcome === "uncertain"
              ? result.error ||
                "Reset result uncertain. Refresh limits before trying again."
              : "This reset credit is no longer available.",
        });
      }
      if (lifetime.active) void reload(accountKey, true);
    } catch (error) {
      if (feedbackCurrent())
        setFeedback((previous) => ({ ...previous, error: errorText(error) }));
    } finally {
      lifetime.resetBusy = false;
      if (lifetime.active) setResetPending(false);
    }
  };
  const phone = useMediaQuery(layoutQueries.navigationCompact);
  const percent = contextProgress?.percent ?? null;
  const refreshing = isRefreshing(selectedAccountKey);
  const confirmReset =
    feedback.key === selectedAccountKey ? feedback.confirm : null;
  const resetError = feedback.key === selectedAccountKey ? feedback.error : "";
  const resetNotice =
    feedback.key === selectedAccountKey ? feedback.notice : "";
  const showAnalytics = () => {
    setContextOpen(false);
    setAnalyticsOpen(true);
  };
  const limitsButton = (
    <Button
      className="limits-toggle account-limits-toggle"
      variant="subtle"
      size="compact-xs"
      aria-label="Account limits"
      title={
        lowAccount
          ? `Low allowance: ${lowAccount.email || lowAccount.label}`
          : sessionCost
            ? `Session estimate: ${dollars(sessionCost.totalUSD)}`
            : undefined
      }
      onClick={() => changeOpened(!opened)}
      leftSection={<Gauge size={13} />}
      rightSection={<ChevronUp size={12} />}
    >
      <span className="account-limits-summary">
        <span className="account-limits-label">Limits</span>
        {lowAccount && (
          <span className="account-limits-warning">Low allowance</span>
        )}
      </span>
    </Button>
  );
  const contextPanel = (
    <>
      <strong>Chat context</strong>
      <p>
        {contextProgress
          ? `${contextProgress.tokens.toLocaleString()} of ${contextProgress.window.toLocaleString()} tokens used (${percent}%).`
          : "No context data yet."}
      </p>
      {detail?.compactions !== undefined && (
        <small>Compacted {detail?.compactions} times.</small>
      )}
      <Button variant="subtle" onClick={showAnalytics}>
        Advanced analytics
      </Button>
    </>
  );
  const limitsPanel = (
    <section
      className="account-limits-panel"
      aria-label="Account limits details"
    >
      <header className="account-limits-heading">
        <div>
          <h3>Account limits</h3>
        </div>
        <Button
          size="compact-xs"
          variant="subtle"
          loading={!activeAccountLimits?.data && refreshing}
          onClick={() => void reload(selectedAccountKey, true)}
          leftSection={<RefreshCw size={13} />}
        >
          Refresh
        </Button>
      </header>
      <AccountTilesLayout
        label="Accounts with limits"
        provider={accountProvider(selectedAccount)}
        providers={[...new Set(visibleUsageAccounts.map(accountProvider))]}
        onProvider={(provider) => {
          const account = visibleUsageAccounts.find(
            (item) => accountProvider(item) === provider,
          );
          if (account) selectAccount(account.id);
        }}
      >
        <div className="account-limits-tiles">
          {[...new Set(visibleUsageAccounts.map(accountProvider))].map(
            (provider) => (
              <Tabs
                key={provider}
                value={
                  accountProvider(selectedAccount) === provider
                    ? selectedAccountKey
                    : (visibleUsageAccounts.find(
                        (item) => accountProvider(item) === provider,
                      )?.id ?? null)
                }
                onChange={(value) => {
                  if (value) selectAccount(value);
                }}
                variant="pills"
                className="account-limits-provider-panel"
                data-active={
                  accountProvider(selectedAccount) === provider || undefined
                }
                aria-hidden={accountProvider(selectedAccount) !== provider}
                inert={accountProvider(selectedAccount) !== provider}
              >
                <Tabs.List aria-label="Accounts with limits">
                  {visibleUsageAccounts.map((item, index) => {
                    if (accountProvider(item) !== provider) return null;
                    const bucketState =
                      projectionFor(item)?.projection.buckets?.current;
                    const remaining = bucketState?.remaining ?? null;
                    return (
                      <Tabs.Tab
                        key={item.id}
                        value={item.id}
                        className="setup-account"
                        data-account-key={item.id}
                        data-selected={
                          item.id === selectedAccountKey || undefined
                        }
                        aria-current={
                          item.id === fallbackKey ? "true" : undefined
                        }
                        id={`usage-account-tab-${index}`}
                        aria-controls="usage-account-panel"
                        title={humanAccountLabel(item)}
                        aria-label={`${humanAccountLabel(item)}${item.provider ? `, ${item.provider}` : ""} account limits`}
                      >
                        <AccountTileLabel
                          name={displayAccountName(item)}
                          nameClassName="account-limits-tile-name"
                          remaining={remaining}
                        />
                        {remaining === null && (
                          <small className="account-remaining">
                            Unavailable
                          </small>
                        )}
                        {item.id === fallbackKey && (
                          <small className="account-limits-current">
                            Current chat
                          </small>
                        )}
                        <AccountRemainingBar
                          remaining={remaining}
                          color={bucketState?.color ?? LimitColor.paused}
                          label={`${humanAccountLabel(item)} remaining limit`}
                        />
                      </Tabs.Tab>
                    );
                  })}
                </Tabs.List>
              </Tabs>
            ),
          )}
        </div>
      </AccountTilesLayout>
      <div className="account-limits-detail-stack">
        {visibleUsageAccounts.map((activeAccount, panelIndex) => {
          const isSelected = activeAccount.id === selectedAccountKey;
          const name = displayAccountName(activeAccount);
          const selectedAccountName =
            name.toLowerCase() === accountProvider(activeAccount) &&
            activeAccount.email
              ? activeAccount.email
              : name;
          const limits = limitsFor(activeAccount);
          const loadingLimits = !limits?.data && isRefreshing(activeAccount.id);
          const buckets =
            projectionFor(activeAccount)?.projection.buckets?.current.buckets ??
            [];
          const recovery =
            projectionFor(activeAccount)?.projection.agents?.[agent.id]
              ?.recovery;
          const resetCredits = projectionFor(activeAccount)?.projection.reset;
          const availableResets = resetCredits?.credits ?? [];
          return (
            <div
              key={activeAccount.id}
              className="account-limits-detail-panel"
              data-active={isSelected || undefined}
              aria-hidden={!isSelected}
              inert={!isSelected}
            >
              <div className="account-limits-selected-name">
                <strong title={selectedAccountName}>
                  {selectedAccountName}
                </strong>
                {activeAccount.plan && (
                  <small className="account-plan">{activeAccount.plan}</small>
                )}
                {!selectedAccount?.email &&
                  (!selectedAccount?.label ||
                    activeAccount.label === activeAccount.id) && (
                    <small>{selectedAccount?.id || fallbackKey}</small>
                  )}
              </div>
              {(projectionFor(activeAccount)?.projection.status.kind ===
                "checking" ||
                projectionFor(activeAccount)?.projection.status.kind ===
                  "pending") && <p role="status">Checking account limits…</p>}
              {recovery && (
                <LimitRecoveryNotice
                  key={JSON.stringify(recovery)}
                  recovery={recovery}
                  tabIndex={isSelected ? undefined : -1}
                  onRefresh={() => reload(activeAccount.id, true)}
                />
              )}
              <div
                className="account-limits-groups"
                role="tabpanel"
                id={
                  isSelected
                    ? "usage-account-panel"
                    : `usage-account-panel-${panelIndex}`
                }
                aria-labelledby={`usage-account-tab-${panelIndex}`}
                aria-label={`${humanAccountLabel(activeAccount)} limits`}
              >
                {buckets.map((bucket) => (
                  <section
                    className="account-limit-group"
                    key={bucket.id}
                    aria-label={`${bucket.name} limits`}
                  >
                    <header>
                      <strong>{bucket.name}</strong>
                      {bucket.planType !== null && (
                        <span>{bucket.planType}</span>
                      )}
                    </header>
                    <div className="account-limit-windows">
                      {bucket.windows.map((window, index) => (
                        <div
                          className="account-limit-window"
                          key={`${window.label}-${index}`}
                        >
                          <div className="account-limit-value">
                            <span>{window.label}</span>
                            <strong
                              className={
                                window.warning ? "account-limits-warning" : ""
                              }
                            >
                              {window.expired ? (
                                "Awaiting update"
                              ) : window.remaining === null ? (
                                "Unavailable"
                              ) : (
                                <>
                                  {formatPercent(window.remaining)}
                                  <small> left</small>
                                </>
                              )}
                            </strong>
                          </div>
                          {window.remaining !== null && !window.expired && (
                            <Progress
                              value={window.remaining}
                              size="xs"
                              color={remainingLimitColor(window.color)}
                              aria-label={`${bucket.name} ${window.label} remaining`}
                            />
                          )}
                          <div className="account-limit-reset">
                            {window.reset ? (
                              <>
                                <span>
                                  {window.expired ? "Reset passed" : "Resets"}
                                </span>
                                <time
                                  dateTime={new Date(
                                    window.reset * 1000,
                                  ).toISOString()}
                                  title={new Date(
                                    window.reset * 1000,
                                  ).toString()}
                                >
                                  {localDateTime(
                                    new Date(window.reset * 1000),
                                    {
                                      month: "short",
                                      day: "numeric",
                                      hour: "2-digit",
                                      minute: "2-digit",
                                    },
                                  )}
                                </time>
                              </>
                            ) : (
                              <span>Reset time unavailable</span>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                    {!bucket.windows.length && (
                      <p className="account-limits-empty">
                        No limit windows reported.
                      </p>
                    )}
                    {bucket.creditsLabel !== null && (
                      <div className="account-limit-credits">
                        <span>Credits</span>
                        <strong>{bucket.creditsLabel}</strong>
                      </div>
                    )}
                  </section>
                ))}
                {!buckets.length && (
                  <p className="account-limits-empty">
                    {loadingLimits
                      ? "Loading account limits…"
                      : "Codex has not supplied account limits."}
                  </p>
                )}
              </div>
              {selectedAccount?.provider === ProviderKind.Claude && (
                <section
                  className="account-reset-credits"
                  aria-label="Claude free limit resets"
                >
                  <header>
                    <strong>Free limit resets</strong>
                    <span>Check on Claude</span>
                  </header>
                  <p>
                    Opens Claude. Use the same account, then confirm “Reset for
                    free”.
                  </p>
                  <div className="account-reset-credit-row">
                    <Button
                      tabIndex={isSelected ? undefined : -1}
                      component="a"
                      href="https://claude.ai/settings/usage"
                      target="_blank"
                      rel="noopener noreferrer"
                      size="compact-xs"
                      variant="default"
                      rightSection={<ExternalLink size={13} />}
                    >
                      Open free resets
                    </Button>
                    <Button
                      tabIndex={isSelected ? undefined : -1}
                      size="compact-xs"
                      variant="subtle"
                      loading={!limits?.data && (refreshing || loadingLimits)}
                      onClick={() => void reload(selectedAccountKey, true)}
                    >
                      Refresh after reset
                    </Button>
                  </div>
                </section>
              )}
              {resetCredits?.present && (
                <section
                  className="account-reset-credits"
                  aria-label="Limit reset credits"
                >
                  <header>
                    <strong>Limit resets</strong>
                    <span>{resetCredits.countLabel}</span>
                  </header>
                  {availableResets.map((credit) => (
                    <div className="account-reset-credit" key={credit.id}>
                      <div className="account-reset-credit-row">
                        <div>
                          <strong title={credit.description ?? undefined}>
                            {credit.title}
                          </strong>
                          <span>
                            {credit.expiresAt !== null ? (
                              <>
                                Expires{" "}
                                <time
                                  dateTime={new Date(
                                    credit.expiresAt * 1000,
                                  ).toISOString()}
                                >
                                  {localDateTime(
                                    new Date(credit.expiresAt * 1000),
                                    {
                                      month: "short",
                                      day: "numeric",
                                      hour: "2-digit",
                                      minute: "2-digit",
                                    },
                                  )}
                                </time>
                              </>
                            ) : (
                              "Expiration unavailable"
                            )}
                          </span>
                        </div>
                        <Button
                          tabIndex={isSelected ? undefined : -1}
                          size="compact-xs"
                          variant="light"
                          disabled={
                            resetPending ||
                            resetApplied.has(
                              resetAppliedKey(credit.id, activeAccount.id),
                            ) ||
                            !credit.enabled
                          }
                          onClick={() => {
                            requestReset(credit.id);
                          }}
                        >
                          {resetApplied.has(
                            resetAppliedKey(credit.id, activeAccount.id),
                          )
                            ? "Applied"
                            : "Apply reset"}
                        </Button>
                      </div>
                      {isSelected && confirmReset === credit.id && (
                        <div
                          className="account-reset-confirm"
                          role="group"
                          aria-label={credit.confirmLabel}
                        >
                          <p>
                            Use this reset credit now? This spends one credit.
                          </p>
                          <div>
                            <Button
                              tabIndex={isSelected ? undefined : -1}
                              size="compact-xs"
                              variant="default"
                              disabled={resetPending}
                              onClick={() => {
                                requestReset(null);
                              }}
                            >
                              Cancel
                            </Button>
                            <Button
                              tabIndex={isSelected ? undefined : -1}
                              size="compact-xs"
                              loading={resetPending}
                              disabled={resetPending || !credit.enabled}
                              onClick={() => void applyReset(credit.id)}
                            >
                              Use one reset credit
                            </Button>
                          </div>
                        </div>
                      )}
                    </div>
                  ))}
                  {resetCredits.emptyMessage && (
                    <p>{resetCredits.emptyMessage}</p>
                  )}
                  {isSelected && resetError && (
                    <p className="account-limits-warning" role="alert">
                      {resetError}
                    </p>
                  )}
                  {isSelected && resetNotice && (
                    <p role="status">{resetNotice}</p>
                  )}
                </section>
              )}
            </div>
          );
        })}
      </div>
      <section className="account-costs" aria-label="Local cost estimates">
        <header>
          <strong>Estimated API cost</strong>
          <span>USD</span>
        </header>
        <div className="account-cost-values">
          {(
            [
              ["Today", costs?.data?.todayUSD],
              ["Last 30 days", costs?.data?.last30DaysUSD],
            ] as const
          ).map(([label, value]) => (
            <div key={label}>
              <span>{label}</span>
              <strong
                className={number(value) ? undefined : "account-cost-missing"}
              >
                {dollars(value)}
              </strong>
            </div>
          ))}
        </div>
        <p className="account-cost-caption" role="status">
          {costs?.refreshing
            ? "Refreshing estimate…"
            : costInterestError || costs?.error
              ? "Refresh failed · stale estimate"
              : costs?.stale
                ? "Stale estimate"
                : "Saved estimate"}
        </p>
        {number(costs?.at) && (
          <p className="account-cost-caption">
            {`Updated ${Math.max(0, Math.round(Date.now() / 1000 - costs.at))} seconds ago`}
          </p>
        )}
        {(costs?.data?.coverage === "partial" ||
          costs?.stale ||
          costs?.error) && (
          <p className="account-cost-caption">
            {costs?.error || costs?.stale
              ? number(costs?.data?.todayUSD) ||
                number(costs?.data?.last30DaysUSD)
                ? "Saved estimate"
                : "Estimate unavailable"
              : "Partial estimate"}
          </p>
        )}
        {typeof costs?.data?.note === "string" && (
          <p className="account-cost-caption">{costs.data.note}</p>
        )}
      </section>
      <footer className="account-limits-updated" role="status">
        {activeAccountLimits?.error
          ? activeAccountLimits?.data
            ? "Saved limits"
            : "Limits temporarily unavailable"
          : number(activeAccountLimits?.at)
            ? "Updated"
            : ""}
        {number(activeAccountLimits?.at) && (
          <span>
            {localTime(new Date(activeAccountLimits!.at * 1000), {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </span>
        )}
      </footer>{" "}
    </section>
  );
  return (
    <div
      className="usage-footer"
      id="usage-footer"
      data-account-key={agent.accountKey || "default"}
    >
      <PassiveUsage>
        <div
          className="session-cost-summary"
          aria-label={
            sessionCost
              ? `Session estimate: ${dollars(sessionCost.totalUSD)}`
              : "Current team session API cost estimate"
          }
          aria-busy={sessionCost == null || sessionCost.refreshing}
          title={[
            sessionCost
              ? `Session estimate: ${dollars(sessionCost.totalUSD)}`
              : "",
            ...Object.entries(sessionCost?.breakdown?.providers || {}).map(
              ([provider, value]) => `${provider}: ${dollars(value)}`,
            ),
            ...(Array.isArray(sessionCost?.unknownModels) &&
            sessionCost.unknownModels.length
              ? [`Unpriced models: ${sessionCost.unknownModels.join(", ")}`]
              : []),
            ...(sessionCost?.claudeHistoryIncomplete
              ? ["Earlier Claude totals may be incomplete."]
              : []),
            typeof sessionCost?.cacheAgeSeconds === "number"
              ? `Updated ${Math.round(sessionCost.cacheAgeSeconds)} seconds ago`
              : "",
            typeof sessionCost?.method === "string" ? sessionCost.method : "",
          ]
            .filter(Boolean)
            .join("\n")}
        >
          {sessionCost?.pricingState === "loading" ? (
            ""
          ) : sessionCost ? (
            <>
              <span className="session-cost-label">Session estimate: </span>
              <span className="session-cost-compact-label" aria-hidden="true">
                <CircleDollarSign size={13} />
              </span>
              <span>
                {dollars(sessionCost.totalUSD)}
                {sessionCost.claudeHistoryIncomplete ||
                sessionCost.unknownModels?.length
                  ? "*"
                  : ""}
              </span>
            </>
          ) : (
            ""
          )}
        </div>
        <TokenRate agent={agent} />
      </PassiveUsage>
      {phone ? (
        <>
          <button
            type="button"
            className="context-use"
            aria-label="Chat context"
            aria-expanded={contextOpen}
            onClick={() => setContextOpen(!contextOpen)}
          >
            {percent === null ? "Context" : `Context ${percent}%`}
          </button>
          <BottomSheet
            opened={contextOpen}
            onClose={() => setContextOpen(false)}
            title="Chat context"
            className="context-sheet"
          >
            {contextPanel}
          </BottomSheet>
        </>
      ) : (
        <Popover
          opened={contextOpen}
          onChange={setContextOpen}
          position="top-start"
          width="min(340px, calc(100vw - 24px))"
          trapFocus
          returnFocus
        >
          <Popover.Target>
            <button
              type="button"
              className="context-use"
              aria-label="Chat context"
              aria-expanded={contextOpen}
              onClick={() => setContextOpen(!contextOpen)}
            >
              {percent === null ? "Context" : `Context ${percent}%`}
            </button>
          </Popover.Target>
          <Popover.Dropdown>{contextPanel}</Popover.Dropdown>
        </Popover>
      )}
      {analyticsOpen && (
        <Suspense fallback={null}>
          <Analytics
            key={agent.id}
            agent={agent}
            onClose={() => setAnalyticsOpen(false)}
          />
        </Suspense>
      )}
      <div className="account-limits-group">
        <div
          className="account-limits-dots"
          role="group"
          aria-label="Account weekly allowance"
        >
          {/* One dot per role (Lead, Workers, Review), not one per account. */}
          {roleAccounts.map(({ role, account: item, automatic }) => {
            const projection = projectionFor(item)?.projection.runway?.current;
            const current = role === "Lead";
            const provider = providerLabel(accountProvider(item));
            const label = humanAccountLabel(item);
            const limits = limitsFor(item);
            const details = [
              `${role}: ${automatic ? "automatic, now " : ""}${label} (${provider})${current ? ", current chat account" : ""}`,
              ...(typeof limits?.error === "string" && !limits?.data
                ? [`Limits unavailable: ${limits.error}`]
                : []),
              ...(projection?.windows ?? []).map(
                (window) =>
                  `${window.name} weekly: ${window.remaining === null ? "remaining allowance unavailable" : `${formatPercent(window.remaining)} left`}. ${window.reset === null ? "Reset time unavailable." : `Resets ${localDateTime(new Date(window.reset * 1000), { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" })}.`}${window.days !== null && Number.isFinite(window.days) ? ` About ${window.days.toFixed(1)} days left at the current rate.` : ""}${window.resetFirst ? " Resets before this allowance runs out." : ""}`,
              ),
              !projection || (projection.days === null && !projection.unlimited)
                ? "Projected days unavailable."
                : projection.unlimited
                  ? "At least 7 days left at the current rate. No weekly consumption recorded."
                  : projection.windows.every((window) => window.resetFirst)
                    ? "At least 7 days left at the current rate after reset."
                    : projection.windows.length > 1
                      ? `About ${projection.days?.toFixed(1)} days left across weekly limits.`
                      : "",
              "Rate uses consumption in the elapsed part of the weekly window.",
            ]
              .filter(Boolean)
              .join("\n");
            return (
              <Tooltip
                key={role}
                label={
                  <span className="account-limits-dot-tooltip">{details}</span>
                }
                multiline
                w={300}
                withArrow
                events={{ hover: true, focus: true, touch: true }}
              >
                <button
                  type="button"
                  className="account-limits-dot-target"
                  data-account-key={item.id}
                  data-role={role}
                  aria-label={`${role}: open ${humanAccountLabel(item)} weekly allowance (${provider}). ${details}`}
                  aria-current={current ? "true" : undefined}
                  aria-haspopup="dialog"
                  aria-expanded={opened && selectedAccountKey === item.id}
                  onClick={() => {
                    selectAccount(item.id);
                    changeOpened(true);
                  }}
                >
                  <span
                    className="account-limits-dot"
                    data-color={projection?.color ?? RunwayColor.gray}
                    data-current={current || undefined}
                    aria-hidden="true"
                  />
                </button>
              </Tooltip>
            );
          })}
        </div>
        {phone ? (
          <>
            {limitsButton}
            <BottomSheet
              opened={opened}
              onClose={() => changeOpened(false)}
              title="Account limits"
              className="account-limits-sheet"
              // The panel keeps one class in a popover and in a phone sheet.
              classNames={{ content: "account-limits-popover" }}
            >
              {limitsPanel}
            </BottomSheet>
          </>
        ) : (
          <Popover
            opened={opened}
            onChange={changeOpened}
            position="top-end"
            width="min(370px, calc(100vw - 24px))"
            withArrow
            shadow="lg"
            trapFocus
            returnFocus
          >
            <Popover.Target>{limitsButton}</Popover.Target>
            <Popover.Dropdown className="account-limits-popover">
              {limitsPanel}
            </Popover.Dropdown>
          </Popover>
        )}
      </div>
    </div>
  );
}

export default observer(Usage);
