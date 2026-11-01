import { LimitColor, RunwayColor } from "../generated/api";
import { PassiveUsage } from "../hooks/BottomInsetBand";
import { useSubscription } from "../sync/subscriptions";
import { randomUUID } from "../uuid";
import { ProviderKind, providerLabel } from "../generated/providerKind.ts";
import { useServerApi } from "../servers/ServerServicesContext";
import { localDateTime, localTime } from "../local-time";
import {
  accountLimits,
  jsonObject,
  type AccountLimitsSnapshot,
} from "../usage/accountUsage";
import { weeklyRunway } from "../usage/weeklyRunway";
import { accountBuckets } from "../usage/accountBuckets";
import { resetCreditEnabled } from "../usage/resetCredits";
import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { Button, Popover, Progress, Tabs, Tooltip } from "@mantine/core";
import {
  ChevronUp,
  ExternalLink,
  Gauge,
  RefreshCw,
  CircleDollarSign,
} from "lucide-react";
import type { Agent, Snapshot } from "../types";
import { errorText, type GetResult } from "../api";
import { limitRecovery } from "../usage/limitRecovery";
import LimitRecoveryNotice from "./LimitRecoveryNotice";
import { useServerSync } from "../servers/ServerServicesContext";
export { limitRecovery } from "../usage/limitRecovery";
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
export type UsageAccount = Pick<Account, "label"> & {
  [Field in "email" | "plan" | "accountId"]?: Account[Field] | undefined;
} & {
  provider?: Account["provider"] | undefined;
  key: string;
  signedOut?: boolean;
  runway?:
    | import("../generated/api").components["schemas"]["WeeklyRunway"]
    | undefined;
  reset?:
    | import("../generated/api").components["schemas"]["ResetProjection"]
    | undefined;
  buckets?:
    | import("../generated/api").components["schemas"]["BucketProjection"]
    | undefined;
  limits: UsageLimitSnapshot | null;
  loading: boolean;
  reload: (force?: boolean) => void | Promise<void>;
};

const humanAccountLabel = (account?: UsageAccount) =>
  account?.email ||
  (account?.label && account.label !== account.key ? account.label : "") ||
  "Account name unavailable";

const displayAccountName = (account?: UsageAccount) =>
  humanAccountLabel(account) === "Account name unavailable"
    ? "Account name unavailable"
    : accountDisplayName({
        label: account?.label === account?.key ? "" : account?.label || "",
        email: account?.email,
      });
const accountProvider = (account?: UsageAccount) => account?.provider || "";

type UsageLimitSnapshot = AccountLimitsSnapshot;

const number = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);
export const formatPercent = (value: number) =>
  value > 0 && value < 1 ? "<1%" : `${Math.floor(value)}%`;
const dollars = (value: unknown) =>
  number(value)
    ? new Intl.NumberFormat(undefined, {
        style: "currency",
        currency: "USD",
        maximumFractionDigits: 2,
      }).format(value)
    : "Unavailable";
function resetIn(reset: number, now: number) {
  const minutes = Math.max(0, Math.ceil((reset - now) / 60));
  if (!minutes) return "Awaiting update";
  if (minutes < 60) return `in ${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `in ${hours}h ${minutes % 60}m`;
  return `in ${Math.floor(hours / 24)}d ${hours % 24}h`;
}
export default function Usage({
  agent,
  stateDir,
  limits: reportedLimits,
  accountLabel,
  reload,
  opened,
  onChange,
  limitsLoading,
  accounts = [],
  visible = true,
}: {
  agent: Agent;
  stateDir: string;
  limits: UsageLimitSnapshot | null;
  accountLabel?: string | undefined;
  reload: () => void | Promise<void>;
  opened?: boolean;
  onChange?: (opened: boolean) => void;
  limitsLoading?: boolean | undefined;
  accounts?: UsageAccount[] | undefined;
  visible?: boolean;
}) {
  const { subscribeStateProjection } = useServerSync();
  const { post } = useServerApi();

  const [localOpened, setLocalOpened] = useState(false);
  const limitsOpened = opened ?? localOpened;
  const changeOpened = (value: boolean) => {
    setLocalOpened(value);
    onChange?.(value);
  };
  const fallbackKey = agent.accountKey || "default";
  const usageAccounts: UsageAccount[] = accounts.length
    ? accounts
    : [
        {
          key: fallbackKey,
          label: accountLabel || "",
          limits: reportedLimits,
          loading: limitsLoading ?? !reportedLimits,
          reload: () => reload(),
        },
      ];
  const [activeAccountKey, setActiveAccountKey] = useState(fallbackKey);
  const activeAccount =
    usageAccounts.find((item) => item.key === activeAccountKey) ||
    usageAccounts[0];
  const selectedAccountKey = activeAccount?.key || fallbackKey;
  const activeAccountReload = useRef(activeAccount?.reload);
  activeAccountReload.current = activeAccount?.reload;
  const autoReloadedAccounts = useRef(new Set<string>());
  const limits = accountLimits(
    activeAccount?.limits,
    selectedAccountKey,
    activeAccount?.accountId,
  );
  const loadingLimits =
    !limits?.data &&
    (activeAccount?.loading ?? limitsLoading ?? !reportedLimits);
  const accountAgent = { ...agent, accountKey: selectedAccountKey };
  const [analyticsOpen, setAnalyticsOpen] = useState(false);
  const [contextOpen, setContextOpen] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const refreshLimits = async () => {
    if (refreshing) return;
    setRefreshing(true);
    try {
      await (activeAccount?.reload(true) ?? reload());
    } finally {
      setRefreshing(false);
    }
  };
  const [confirmReset, setConfirmReset] = useState<string | null>(null);
  const [resetPending, setResetPending] = useState(false);
  const [resetError, setResetError] = useState("");
  const [resetNotice, setResetNotice] = useState("");
  const [resetApplied, setResetApplied] = useState<string[]>([]);
  const resetAppliedKey = (creditId: string, accountKey = selectedAccountKey) =>
    `${accountKey}:${creditId}`;
  useEffect(() => {
    setConfirmReset(null);
    setResetError("");
    setResetNotice("");
  }, [selectedAccountKey]);
  const resetLock = useRef(false);
  const resetAttempts = useRef(new Map<string, string>());
  const applyReset = async (creditId: string) => {
    if (resetLock.current || resetApplied.includes(resetAppliedKey(creditId)))
      return;
    resetLock.current = true;
    setResetPending(true);
    setResetError("");
    setResetNotice("");
    try {
      const accountId = activeAccount?.reset?.accountId;
      if (!accountId)
        throw new Error("Refresh limits before using this reset credit.");
      const key = `${accountId}:${creditId}`;
      const requestId = resetAttempts.current.get(key) || randomUUID();
      resetAttempts.current.set(key, requestId);
      const result = await post("/api/limits/reset", {
        credit_id: creditId,
        account_id: accountId,
        account_key: selectedAccountKey,
        request_id: requestId,
      });
      switch (result.outcome) {
        case "reset":
        case "alreadyRedeemed":
          resetAttempts.current.delete(key);
          setResetApplied((ids) => [...ids, resetAppliedKey(creditId)]);
          setConfirmReset(null);
          setResetNotice(
            result.outcome === "reset"
              ? "Reset applied."
              : "This reset credit was already used.",
          );
          break;
        case "nothingToReset":
          resetAttempts.current.delete(key);
          setConfirmReset(null);
          setResetNotice("No limits need a reset.");
          break;
        case "noCredit":
          resetAttempts.current.delete(key);
          setResetError("This reset credit is no longer available.");
          break;
        case "uncertain":
          setResetError(
            result.error ||
              "Reset result uncertain. Refresh limits before trying again.",
          );
          break;
      }
      void (activeAccount?.reload(true) ?? reload());
    } catch (error) {
      setResetError(errorText(error));
    } finally {
      resetLock.current = false;
      setResetPending(false);
    }
  };
  const accountKey = selectedAccountKey;
  const [costs, setCosts] = useState<
    (Partial<GetResult<"/api/costs">> & { error?: string | null }) | null
  >(null);
  const rootId = agent.rootId || agent.id;
  useSubscription(visible ? { kind: "session-cost", agentId: rootId } : null);
  useSubscription(visible && limitsOpened ? { kind: "costs" } : null);
  const costObservation = useRef<{
    scope: string;
    signature: string | null;
  } | null>(null);
  const costInterest = useRef<{
    scope: string;
    accountKey: string;
    baseline: string | null;
    phase: "baseline" | "pending" | "settled";
  } | null>(null);
  const [costInterestError, setCostInterestError] = useState("");
  const sendCostInterest = useCallback(
    (attempt: NonNullable<typeof costInterest.current>) => {
      if (costInterest.current !== attempt || attempt.phase !== "baseline")
        return;
      attempt.phase = "pending";
      void post("/api/costs/interest", {
        account_key: attempt.accountKey,
      }).catch((error) => {
        if (costInterest.current === attempt && attempt.phase === "pending")
          setCostInterestError(errorText(error));
      });
    },
    [post],
  );
  useEffect(() => {
    if (!visible || !limitsOpened) {
      costInterest.current = null;
      return;
    }
    const scope = JSON.stringify([stateDir, accountKey]);
    if (costInterest.current?.scope === scope) return;
    const observation =
      costObservation.current?.scope === scope ? costObservation.current : null;
    const attempt: NonNullable<typeof costInterest.current> = {
      scope,
      accountKey,
      baseline: observation?.signature ?? null,
      phase: "baseline",
    };
    costInterest.current = attempt;
    setCostInterestError("");
    // A local projection baseline distinguishes cached data from the result
    // of this opening, including when switching accounts while already open.
    if (observation) sendCostInterest(attempt);
  }, [visible, limitsOpened, stateDir, accountKey, sendCostInterest]);
  const costScope = JSON.stringify([stateDir, rootId]);
  const [sessionCostState, setSessionCostState] = useState<{
    scope: string;
    source: typeof subscribeStateProjection;
    value:
      | NonNullable<Snapshot["runtime"]["sessionCostsByAgent"]>[string]
      | null;
  } | null>(null);
  const sessionCost =
    sessionCostState?.scope === costScope &&
    sessionCostState.source === subscribeStateProjection
      ? sessionCostState.value
      : null;
  const sessionCostUpdating = sessionCost === null || sessionCost.refreshing;
  useEffect(() => {
    let active = true;
    const stop = subscribeStateProjection(
      (snapshot) => {
        if (!active) return;
        const value = snapshot?.runtime.sessionCostsByAgent?.[rootId];
        setSessionCostState({
          scope: costScope,
          source: subscribeStateProjection,
          value: value?.rootId === rootId ? value : null,
        });
      },
      () => {},
    );
    return () => {
      active = false;
      stop();
    };
  }, [costScope, rootId, subscribeStateProjection]);
  useEffect(() => {
    setCosts(null);
    const stop = subscribeStateProjection(
      (snapshot) => {
        const value = snapshot?.runtime.accountCostsByAccount?.[accountKey];
        if (!snapshot) return;
        const scope = JSON.stringify([stateDir, accountKey]);
        const signature = value ? JSON.stringify(value) : null;
        costObservation.current = { scope, signature };
        const attempt = costInterest.current;
        if (attempt?.scope === scope) {
          if (attempt.phase === "baseline") {
            attempt.baseline = signature;
            sendCostInterest(attempt);
          } else if (
            attempt.phase === "pending" &&
            signature !== attempt.baseline
          ) {
            attempt.phase = "settled";
            setCostInterestError("");
          }
        }
        setCosts(value || null);
      },
      () => {},
    );
    return () => {
      costObservation.current = null;
      stop();
    };
  }, [stateDir, accountKey, subscribeStateProjection, sendCostInterest]);
  const [now, setNow] = useState(() => Date.now() / 1000);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now() / 1000), 30000);
    return () => window.clearInterval(timer);
  }, []);
  const contextUsage = jsonObject(agent.contextUsage);
  const contextTokens = contextUsage?.tokens;
  const contextWindow = contextUsage?.window;
  const known =
    number(contextTokens) && number(contextWindow) && contextWindow > 0;
  const percent = known
    ? Math.round((contextTokens / contextWindow) * 100)
    : null;
  // The current chat's model decision stays with its mounted account panel,
  // including while another account is selected. Never request that decision
  // using the selected account's unrelated limit snapshot.
  const currentAccount = usageAccounts.find((item) => item.key === fallbackKey);
  const currentLimits = accountLimits(
    currentAccount?.limits,
    fallbackKey,
    currentAccount?.accountId,
  );
  const modelLimits = currentLimits;
  const selectedLimits =
    selectedAccountKey === fallbackKey ? modelLimits : limits;
  const selectedRecovery = limitRecovery(accountAgent, selectedLimits, now);
  useEffect(() => {
    autoReloadedAccounts.current.clear();
  }, [agent.id]);
  useEffect(() => {
    if (!limitsOpened || !activeAccount || selectedAccountKey === fallbackKey)
      return;
    if (autoReloadedAccounts.current.has(selectedAccountKey)) return;
    autoReloadedAccounts.current.add(selectedAccountKey);
    void activeAccountReload.current?.(false);
  }, [limitsOpened, selectedAccountKey, fallbackKey]);
  const lowAccount = usageAccounts.find(
    (item) => accountBuckets(item.buckets, now)?.low,
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
          aria-busy={sessionCostUpdating}
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
        <Popover.Dropdown>
          <strong>Chat context</strong>
          <p>
            {known
              ? `${contextTokens.toLocaleString()} of ${contextWindow.toLocaleString()} tokens used (${percent}%).`
              : "No context data yet."}
          </p>
          {agent.compactions !== undefined && (
            <small>Compacted {agent.compactions} times.</small>
          )}
          <Button
            variant="subtle"
            onClick={() => {
              setContextOpen(false);
              setAnalyticsOpen(true);
            }}
          >
            Advanced analytics
          </Button>
        </Popover.Dropdown>
      </Popover>
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
          {usageAccounts.map((item) => {
            const projection = item.signedOut
              ? null
              : weeklyRunway(item.runway, now);
            const current = item.key === fallbackKey;
            const provider = providerLabel(accountProvider(item));
            const label = humanAccountLabel(item);
            const details = [
              `${label} (${provider})${current ? ", current chat account" : ""}`,
              ...(item.signedOut
                ? ["Signed out. Weekly allowance unavailable."]
                : []),
              ...(!item.signedOut &&
              typeof item.limits?.error === "string" &&
              !item.limits?.data
                ? [`Limits unavailable: ${item.limits.error}`]
                : []),
              ...(projection?.windows ?? []).map(
                (window) =>
                  `${window.name} weekly: ${window.remaining === null ? "remaining allowance unavailable" : `${formatPercent(window.remaining)} left`}. ${window.reset === null ? "Reset time unavailable." : window.reset <= now ? "Cached reset passed." : `Resets ${localDateTime(new Date(window.reset * 1000), { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" })}.`}${window.days !== null && Number.isFinite(window.days) ? ` About ${window.days.toFixed(1)} days left at the current rate.` : ""}${window.resetFirst ? " Resets before this allowance runs out." : ""}`,
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
                key={item.key}
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
                  data-account-key={item.key}
                  aria-label={`Open ${humanAccountLabel(item)} weekly allowance (${provider}). ${details}`}
                  aria-current={current ? "true" : undefined}
                  aria-haspopup="dialog"
                  aria-expanded={
                    limitsOpened && selectedAccountKey === item.key
                  }
                  onClick={() => {
                    setActiveAccountKey(item.key);
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
        <Popover
          opened={limitsOpened}
          onChange={changeOpened}
          position="top-end"
          width="min(370px, calc(100vw - 24px))"
          withArrow
          shadow="lg"
          trapFocus
          returnFocus
        >
          <Popover.Target>
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
              onClick={() => changeOpened(!limitsOpened)}
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
          </Popover.Target>
          <Popover.Dropdown className="account-limits-popover">
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
                  loading={!limits?.data && (refreshing || loadingLimits)}
                  onClick={() =>
                    void (activeAccount?.reload(true) ?? refreshLimits())
                  }
                  leftSection={<RefreshCw size={13} />}
                >
                  Refresh
                </Button>
              </header>
              <AccountTilesLayout
                label="Accounts with limits"
                provider={accountProvider(activeAccount)}
                providers={[...new Set(usageAccounts.map(accountProvider))]}
                onProvider={(provider) => {
                  const account = usageAccounts.find(
                    (item) => accountProvider(item) === provider,
                  );
                  if (account) setActiveAccountKey(account.key);
                }}
              >
                <div className="account-limits-tiles">
                  {[...new Set(usageAccounts.map(accountProvider))].map(
                    (provider) => (
                      <Tabs
                        key={provider}
                        value={
                          accountProvider(activeAccount) === provider
                            ? selectedAccountKey
                            : (usageAccounts.find(
                                (item) => accountProvider(item) === provider,
                              )?.key ?? null)
                        }
                        onChange={(value) => {
                          if (value) setActiveAccountKey(value);
                        }}
                        variant="pills"
                        className="account-limits-provider-panel"
                        data-active={
                          accountProvider(activeAccount) === provider ||
                          undefined
                        }
                        aria-hidden={
                          accountProvider(activeAccount) !== provider
                        }
                        inert={accountProvider(activeAccount) !== provider}
                      >
                        <Tabs.List aria-label="Accounts with limits">
                          {usageAccounts.map((item, index) => {
                            if (accountProvider(item) !== provider) return null;
                            const bucketState = accountBuckets(
                              item.buckets,
                              now,
                            );
                            const remaining = bucketState?.remaining ?? null;
                            return (
                              <Tabs.Tab
                                key={item.key}
                                value={item.key}
                                className="setup-account"
                                data-account-key={item.key}
                                data-selected={
                                  item.key === selectedAccountKey || undefined
                                }
                                aria-current={
                                  item.key === fallbackKey ? "true" : undefined
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
                                {item.key === fallbackKey && (
                                  <small className="account-limits-current">
                                    Current chat
                                  </small>
                                )}
                                <AccountRemainingBar
                                  remaining={remaining}
                                  color={
                                    bucketState?.color ?? LimitColor.paused
                                  }
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
                {usageAccounts.map((activeAccount, panelIndex) => {
                  const isSelected = activeAccount.key === selectedAccountKey;
                  const name = displayAccountName(activeAccount);
                  const selectedAccountName =
                    name.toLowerCase() === accountProvider(activeAccount) &&
                    activeAccount.email
                      ? activeAccount.email
                      : name;
                  const limits = accountLimits(
                    activeAccount.limits,
                    activeAccount.key,
                    activeAccount.accountId,
                  );
                  const loadingLimits = !limits?.data && activeAccount.loading;
                  const buckets =
                    accountBuckets(activeAccount.buckets, now)?.buckets ?? [];
                  const recovery = isSelected
                    ? selectedRecovery
                    : limitRecovery(
                        { ...agent, accountKey: activeAccount.key },
                        activeAccount.key === fallbackKey
                          ? modelLimits
                          : limits,
                        now,
                      );
                  const resetCredits = activeAccount.reset;
                  const availableResets = resetCredits?.credits ?? [];
                  return (
                    <div
                      key={activeAccount.key}
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
                          <small className="account-plan">
                            {activeAccount.plan}
                          </small>
                        )}
                        {!activeAccount?.email &&
                          (!activeAccount?.label ||
                            activeAccount.label === activeAccount.key) && (
                            <small>{activeAccount?.key || fallbackKey}</small>
                          )}
                      </div>
                      {recovery && (
                        <LimitRecoveryNotice
                          key={JSON.stringify(recovery)}
                          recovery={recovery}
                          tabIndex={isSelected ? undefined : -1}
                          onRefresh={() =>
                            activeAccount.reload(true) ?? refreshLimits()
                          }
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
                                        window.warning
                                          ? "account-limits-warning"
                                          : ""
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
                                  {window.remaining !== null &&
                                    !window.expired && (
                                      <Progress
                                        value={window.remaining}
                                        size="xs"
                                        color={remainingLimitColor(
                                          window.color,
                                        )}
                                        aria-label={`${bucket.name} ${window.label} remaining`}
                                      />
                                    )}
                                  <div className="account-limit-reset">
                                    {window.reset ? (
                                      <>
                                        <span>
                                          {window.expired
                                            ? "Reset passed"
                                            : `Resets ${resetIn(window.reset, now)}`}
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
                      {activeAccount?.provider === ProviderKind.Claude && (
                        <section
                          className="account-reset-credits"
                          aria-label="Claude free limit resets"
                        >
                          <header>
                            <strong>Free limit resets</strong>
                            <span>Check on Claude</span>
                          </header>
                          <p>
                            Opens Claude. Use the same account, then confirm
                            “Reset for free”.
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
                              loading={
                                !limits?.data && (refreshing || loadingLimits)
                              }
                              onClick={() => void refreshLimits()}
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
                            <div
                              className="account-reset-credit"
                              key={credit.id}
                            >
                              <div className="account-reset-credit-row">
                                <div>
                                  <strong
                                    title={credit.description ?? undefined}
                                  >
                                    {credit.title}
                                  </strong>
                                  <span>
                                    {credit.expiresAt !== null ? (
                                      <>
                                        Expires {resetIn(credit.expiresAt, now)}{" "}
                                        ·{" "}
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
                                    resetApplied.includes(
                                      resetAppliedKey(
                                        credit.id,
                                        activeAccount.key,
                                      ),
                                    ) ||
                                    !resetCreditEnabled(credit, now)
                                  }
                                  onClick={() => {
                                    setConfirmReset(credit.id);
                                    setResetError("");
                                  }}
                                >
                                  {resetApplied.includes(
                                    resetAppliedKey(
                                      credit.id,
                                      activeAccount.key,
                                    ),
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
                                    Use this reset credit now? This spends one
                                    credit.
                                  </p>
                                  <div>
                                    <Button
                                      tabIndex={isSelected ? undefined : -1}
                                      size="compact-xs"
                                      variant="default"
                                      disabled={resetPending}
                                      onClick={() => {
                                        setConfirmReset(null);
                                        setResetError("");
                                      }}
                                    >
                                      Cancel
                                    </Button>
                                    <Button
                                      tabIndex={isSelected ? undefined : -1}
                                      size="compact-xs"
                                      loading={resetPending}
                                      disabled={
                                        resetPending ||
                                        !resetCreditEnabled(credit, now)
                                      }
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
              <section
                className="account-costs"
                aria-label="Local cost estimates"
              >
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
                        className={
                          number(value) ? undefined : "account-cost-missing"
                        }
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
                  {number(costs?.at) &&
                    ` · Updated ${Math.max(0, Math.round(now - costs!.at!))} seconds ago`}
                </p>
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
                {limits?.error
                  ? limits?.data
                    ? "Saved limits"
                    : "Limits temporarily unavailable"
                  : number(limits?.at)
                    ? "Updated"
                    : ""}
                {number(limits?.at) && (
                  <span>
                    {localTime(new Date(limits!.at * 1000), {
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </span>
                )}
              </footer>{" "}
            </section>
          </Popover.Dropdown>
        </Popover>
      </div>
    </div>
  );
}
