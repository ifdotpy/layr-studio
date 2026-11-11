import { UsageCosts } from "../UsageCosts/UsageCosts.tsx";
import { UsageTime } from "../UsageTime/UsageTime.tsx";
import {
  Button,
  type ButtonProps,
  Popover,
  Progress,
  Tabs,
  Tooltip,
} from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import {
  ChevronUp,
  CircleDollarSign,
  ExternalLink,
  Gauge,
  RefreshCw,
} from "lucide-react";
import { makeAutoObservable, observable, reaction } from "mobx";
import { observer } from "mobx-react-lite";
import { type ReactNode, Suspense, lazy, useState } from "react";
import { accountDisplayName } from "../../../accountName.ts";
import { errorText } from "../../../api.ts";
import { LimitColor, RunwayColor } from "../../../generated/api.ts";
import {
  type SessionCostData,
  agentId,
} from "../../../generated/observableResources.ts";
import {
  ProviderKind,
  providerLabel,
} from "../../../generated/providerKind.ts";
import { layoutQueries } from "../../../layout/layout-contract.generated.ts";
import { localDateTime, localTime } from "../../../local-time.ts";
import { whileObserved } from "../../../observed.ts";
import { resourceData } from "../../../store/runtime.ts";
import { type Agent } from "../../../types.ts";
import { accountLimits, jsonObject } from "../../../usage/accountUsage.ts";
import { LimitRefresh } from "../../../usage/useLimitRefresh.ts";
import { AccountRemainingBar } from "../../accounts/AccountRemainingBar/AccountRemainingBar.tsx";
import { AccountTileLabel } from "../../accounts/AccountTileLabel/AccountTileLabel.tsx";
import { AccountTilesLayout } from "../../accounts/AccountTilesLayout/AccountTilesLayout.tsx";
import { type Account } from "../../accounts/support/Accounts.tsx";
import { remainingLimitColor } from "../../accounts/usage/support/accountLimitColor.ts";
import {
  useServerApi,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { type AnalyticsSelection } from "../../../analytics/AnalyticsStore.ts";
import { finite as number } from "../../../analytics/format.ts";
import { PassiveUsage } from "../../shell/PassiveUsage/PassiveUsage.tsx";
import BottomSheet from "../../ui/BottomSheet/BottomSheet.tsx";
import LimitRecoveryNotice from "../LimitRecoveryNotice/LimitRecoveryNotice.tsx";
import TokenRate from "../TokenRate/TokenRate.tsx";
import { type RoleAccount, accountsForRoles } from "./roleAccounts.ts";

type UsageApi = ReturnType<typeof useServerApi>;

type UsageModels = ReturnType<typeof useServerStore>;

export const resetKey = (credit: string, account: string) =>
  account + ":" + credit;

type UsageIdentity = Pick<Account, "id" | "label" | "email" | "provider">;

export const humanAccountLabel = (account?: UsageIdentity) =>
  account?.email ||
  (account?.label && account.label !== account.id ? account.label : "") ||
  "Account name unavailable";

export const displayAccountName = (account?: UsageIdentity) =>
  humanAccountLabel(account) === "Account name unavailable"
    ? "Account name unavailable"
    : accountDisplayName({
        label: account?.label === account?.id ? "" : account?.label || "",
        email: account?.email,
      });

export const accountProvider = (account?: UsageIdentity) =>
  account?.provider || "";

export const formatPercent = (value: number) =>
  value > 0 && value < 1 ? "<1%" : `${Math.floor(value)}%`;

export const dollarFormatter = new Intl.NumberFormat(undefined, {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 2,
});

export const dollars = (value: unknown) =>
  number(value) ? dollarFormatter.format(value) : "Unavailable";

export function createUsageView(
  api: Pick<UsageApi, "post">,
  store: Pick<UsageModels, "state" | "costs">,
  fallbackKey = () => "default",
) {
  let stop: (() => void) | undefined;
  const state = makeAutoObservable(
    {
      active: true,
      analyticsQuery: null as AnalyticsSelection | null,
      browse: { key: null as string | null, request: 0 },
      refresh: null as LimitRefresh | null,
      setAnalyticsQuery(value: AnalyticsSelection | null) {
        this.analyticsQuery = value;
      },
      reload(
        read: ConstructorParameters<typeof LimitRefresh>[0],
        key: string,
        force = false,
      ) {
        this.refresh ??= new LimitRefresh(read);
        return this.refresh.reload(key, force);
      },
      isRefreshing(key: string) {
        return this.refresh?.isRefreshing(key) ?? false;
      },
      selectAccount(key: string | null, selected: string, request: number) {
        if (!key || key === selected) return;
        this.browse = { key, request };
        this.setFeedback({ key, confirm: null, error: "", notice: "" });
      },
      requestReset(key: string, confirm: string | null) {
        this.setFeedback({ key, confirm, error: "", notice: "" });
      },
      showAnalytics(agent: Agent, owner: UsageModels, api: UsageApi) {
        const query = {
          agent: agent.id,
          scope: "agent" as const,
          limit: 30,
          offset: 0,
          to_time: Math.floor(Date.now() / 1000),
        };
        const read = owner.reads.analytics(api, query);
        void read.read().catch(() => {});
        this.analyticsQuery = { query, read };
      },
      interests: observable.map<string, ReturnType<typeof createCostInterest>>(
        {},
        { deep: false },
      ),
      resetPending: false,
      resetApplied: new Set<string>(),
      feedback: {
        key: "",
        confirm: null as string | null,
        error: "",
        notice: "",
      },
      *applyReset(
        api: UsageApi,
        owner: UsageModels,
        account: Account | undefined,
        creditId: string,
      ): Generator<
        ReturnType<typeof api.post<"/api/limits/reset">>,
        void,
        Awaited<ReturnType<typeof api.post<"/api/limits/reset">>>
      > {
        if (
          state.resetPending ||
          !account ||
          state.resetApplied.has(resetKey(creditId, account.id))
        )
          return;
        const accountKey = account.id;
        const feedbackCurrent = () =>
          state.active && state.feedback.key === accountKey;
        state.resetPending = true;
        state.requestReset(accountKey, creditId);
        try {
          const accountId = resourceData(
            owner.state().current,
          )?.rateLimitsByAccount.get(account.id)?.projection.reset.accountId;
          if (!accountId)
            throw new Error("Refresh limits before using this reset credit.");
          const key = "usage-reset:" + JSON.stringify([accountId, creditId]);
          const requestId = api.stableRequestId(key);
          const result = yield api.post("/api/limits/reset", {
            credit_id: creditId,
            account_id: accountId,
            account_key: accountKey,
            request_id: requestId,
          });
          if (result.outcome !== "uncertain") api.clearStableRequestId(key);
          if (
            result.outcome === "reset" ||
            result.outcome === "alreadyRedeemed"
          )
            state.resetApplied.add(resetKey(creditId, accountKey));
          if (feedbackCurrent()) {
            const notice = {
              reset: "Reset applied.",
              alreadyRedeemed: "This reset credit was already used.",
              nothingToReset: "No limits need a reset.",
              noCredit: null,
              uncertain: null,
            }[result.outcome];
            state.setFeedback({
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
          if (state.active) void state.reload(api.get, accountKey, true);
        } catch (error) {
          if (feedbackCurrent())
            state.setFeedback({ ...state.feedback, error: errorText(error) });
        } finally {
          state.resetPending = false;
        }
      },
      setFeedback(value: typeof state.feedback) {
        this.feedback = value;
      },
      get accounts() {
        return (
          resourceData(store.state().current)?.accountsSummary.get("current")
            ?.accounts ?? []
        );
      },
      get lowAccount(): Account | undefined {
        return this.accounts.find(
          (item) => this.projectionFor(item)?.projection.buckets?.current.low,
        );
      },
      get providers() {
        const groups = new Map<string, { account: Account; index: number }[]>();
        this.accounts.forEach((account, index) => {
          const provider = accountProvider(account);
          const group = groups.get(provider) ?? [];
          group.push({ account, index });
          groups.set(provider, group);
        });
        return [...groups].map(([provider, accounts]) => ({
          provider,
          accounts,
        }));
      },
      projectionFor(account: Account) {
        return resourceData(store.state().current)?.rateLimitsByAccount.get(
          account.id,
        );
      },
      limitsFor(account: Account) {
        return accountLimits(
          this.projectionFor(account)?.observation,
          account.id,
          account.accountId,
        );
      },
      start() {
        this.active = true;
        stop ??= reaction(
          () => [
            ...new Set([
              fallbackKey(),
              ...this.accounts.map((account) => account.id),
            ]),
          ],
          (keys) => {
            for (const key of keys)
              if (!this.interests.has(key))
                this.interests.set(key, createCostInterest(api, store, key));
            for (const [key, interest] of this.interests)
              if (!keys.includes(key)) {
                interest.dispose();
                this.interests.delete(key);
              }
          },
          { fireImmediately: true },
        );
      },
      dispose() {
        this.active = false;
        stop?.();
        stop = undefined;
        for (const interest of this.interests.values()) interest.dispose();
        this.interests.clear();
      },
    },
    {},
    { autoBind: true },
  );
  return state;
}

export function createCostInterest(
  api: Pick<UsageApi, "post">,
  store: Pick<UsageModels, "costs">,
  key: string | null,
) {
  let generation = 0;
  let stop: (() => void) | undefined;
  const signature = () => {
    const rows = resourceData(store.costs().current);
    return rows === undefined
      ? undefined
      : JSON.stringify(rows.find((row) => row.accountKey === key) ?? null);
  };
  const state = makeAutoObservable(
    {
      error: "",
      *load() {
        if (!key) return;
        const attempt = ++generation;
        let settled = false;
        this.error = "";
        let baseline = signature();
        stop = reaction(signature, (next) => {
          if (baseline === undefined) baseline = next;
          else if (next !== undefined && next !== baseline) {
            settled = true;
            state.error = "";
          }
        });
        try {
          yield api.post("/api/costs/interest", { account_key: key });
        } catch (error) {
          if (attempt === generation && !settled) this.error = errorText(error);
        }
      },
      dispose() {
        generation++;
        stop?.();
        stop = undefined;
      },
    },
    {},
    { autoBind: true },
  );
  whileObserved(state, "error", () => {
    void state.load();
    return () => state.dispose();
  });
  return state;
}

export type UsageModel = ReturnType<typeof createUsageView>;

export function accountSelection(ui: UsageModel, agent: Agent, request = 0) {
  const fallbackKey = agent.accountKey || "default";
  const wanted =
    (ui.browse.request === request && ui.browse.key) || fallbackKey;
  const account =
    ui.accounts.find((account) => account.id === wanted) || ui.accounts[0];
  return { account, key: account?.id || fallbackKey, fallbackKey };
}

export function contextProgress(value: Parameters<typeof jsonObject>[0]) {
  const context = jsonObject(value),
    tokens = context?.tokens,
    window = context?.window;
  return number(tokens) && number(window) && window > 0
    ? { tokens, window, percent: Math.round((tokens / window) * 100) }
    : null;
}

export function roleDetails(
  { role, account: item, automatic }: RoleAccount<Account>,
  view: ReturnType<UsageModel["projectionFor"]>,
) {
  const current = role === "Lead",
    provider = providerLabel(accountProvider(item)),
    label = humanAccountLabel(item);
  const projection = view?.projection.runway?.current;
  const limits = accountLimits(view?.observation, item.id, item.accountId);
  return [
    `${role}: ${automatic ? "automatic, now " : ""}${label} (${provider})${current ? ", current chat account" : ""}`,
    typeof limits?.error === "string" &&
      !limits?.data &&
      `Limits unavailable: ${limits.error}`,
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
}

export function sessionTitle(sessionCost: SessionCostData | undefined) {
  return [
    sessionCost ? `Session estimate: ${dollars(sessionCost.totalUSD)}` : "",
    ...Object.entries(sessionCost?.breakdown?.providers || {}).map(
      ([provider, value]) => `${provider}: ${dollars(value)}`,
    ),
    sessionCost?.unknownModels?.length &&
      `Unpriced models: ${sessionCost.unknownModels.join(", ")}`,
    sessionCost?.claudeHistoryIncomplete &&
      "Earlier Claude totals may be incomplete.",
    typeof sessionCost?.cacheAgeSeconds === "number"
      ? `Updated ${Math.round(sessionCost.cacheAgeSeconds)} seconds ago`
      : "",
    typeof sessionCost?.method === "string" ? sessionCost.method : "",
  ]
    .filter(Boolean)
    .join("\n");
}

const surfaces = {
  context: ["Chat context", "context", 340, "top-start"],
  limits: ["Account limits", "account-limits", 370, "top-end"],
} as const;

function UsageDisclosure(props: {
  kind: keyof typeof surfaces;
  phone: boolean | undefined;
  opened: boolean;
  onChange: (opened: boolean) => void;
  target: ReactNode;
  children: ReactNode;
}) {
  const [title, name, width, position] = surfaces[props.kind];
  const dropdown =
    props.kind === "limits" ? "account-limits-popover" : undefined;
  if (props.phone)
    return (
      <>
        {props.target}
        <BottomSheet
          opened={props.opened}
          onClose={() => props.onChange(false)}
          title={title}
          className={name + "-sheet"}
          classNames={dropdown ? { content: dropdown } : {}}
        >
          {props.children}
        </BottomSheet>
      </>
    );
  return (
    <Popover
      opened={props.opened}
      onChange={props.onChange}
      position={position}
      width={`min(${width}px, calc(100vw - 24px))`}
      withArrow={!!dropdown}
      {...(dropdown ? { shadow: "lg" } : {})}
      trapFocus
      returnFocus
    >
      <Popover.Target>{props.target}</Popover.Target>
      <Popover.Dropdown className={dropdown}>{props.children}</Popover.Dropdown>
    </Popover>
  );
}

const Analytics = lazy(() => import("../../shell/Analytics/Analytics.tsx"));

export type UsageProps = {
  agent: Agent;
  opened?: boolean | undefined;
  onChange?: ((opened: boolean) => void) | undefined;
  openRequest?: number;
  visible?: boolean;
  usageStore: ReturnType<typeof createUsageView>;
};

export type UsageControls = UsageProps & {
  opened: boolean;
  onChange: (opened: boolean) => void;
};

type UsageAccountProps = UsageControls & { account: Account; index: number };

function Usage(props: UsageProps) {
  const [localOpen, setLocalOpen] = useState(props.opened ?? false);
  const controls = {
    ...props,
    opened: props.onChange ? (props.opened ?? false) : localOpen,
    onChange: props.onChange ?? setLocalOpen,
  };
  return (
    <div
      className="usage-footer"
      id="usage-footer"
      data-account-key={props.agent.accountKey || "default"}
    >
      <PassiveUsage>
        <SessionUsage {...props} />
        <TokenRate agent={props.agent} />
      </PassiveUsage>
      <ContextUsage {...props} />
      <AnalyticsDialog {...props} />
      <div className="account-limits-group">
        <RoleDots {...controls} />
        <LimitsDisclosure {...controls} />
      </div>
    </div>
  );
}

const SessionUsage = observer(function SessionUsage(props: UsageProps) {
  const { agent, visible = true } = props;
  const store = useServerStore();
  const sessionCost = visible
    ? resourceData(store.sessionCost(agentId(agent.rootId || agent.id)).current)
    : undefined;
  return (
    <div
      className="session-cost-summary"
      aria-label={
        sessionCost
          ? `Session estimate: ${dollars(sessionCost.totalUSD)}`
          : "Current team session API cost estimate"
      }
      aria-busy={sessionCost == null || sessionCost.refreshing}
      title={sessionTitle(sessionCost)}
    >
      {sessionCost && sessionCost.pricingState !== "loading" && (
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
      )}
    </div>
  );
});

const ContextUsage = observer(function ContextUsage(props: UsageProps) {
  const { agent, usageStore } = props;
  const store = useServerStore(),
    api = useServerApi();
  const detail = resourceData(store.agentDetail(agentId(agent.id)).current);
  const progress = contextProgress(detail?.contextUsage);
  const [contextOpen, setContextOpen] = useState(false);
  const phone = useMediaQuery(layoutQueries.navigationCompact);
  const showAnalytics = () => {
    setContextOpen(false);
    usageStore.showAnalytics(agent, store, api);
  };
  return (
    <UsageDisclosure
      phone={phone}
      opened={contextOpen}
      onChange={setContextOpen}
      kind="context"
      target={
        <button
          type="button"
          className="context-use"
          aria-label="Chat context"
          aria-expanded={contextOpen}
          onClick={() => setContextOpen(!contextOpen)}
        >
          {progress === null ? "Context" : `Context ${progress.percent}%`}
        </button>
      }
    >
      <strong>Chat context</strong>
      <p>
        {progress
          ? `${progress.tokens.toLocaleString()} of ${progress.window.toLocaleString()} tokens used (${progress.percent}%).`
          : "No context data yet."}
      </p>
      {detail?.compactions !== undefined && (
        <small>Compacted {detail?.compactions} times.</small>
      )}
      <Button variant="subtle" onClick={showAnalytics}>
        Advanced analytics
      </Button>
    </UsageDisclosure>
  );
});

const AnalyticsDialog = observer(function AnalyticsDialog({
  agent,
  usageStore,
}: UsageProps) {
  const { analyticsQuery, setAnalyticsQuery } = usageStore;
  return (
    analyticsQuery && (
      <Suspense fallback={null}>
        <Analytics
          key={agent.id}
          agent={agent}
          selection={analyticsQuery}
          onClose={() => setAnalyticsQuery(null)}
        />
      </Suspense>
    )
  );
});

const RoleDots = observer(function RoleDots(props: UsageControls) {
  const { agent, usageStore: ui, openRequest, opened, onChange } = props;
  const store = useServerStore();
  const detail = resourceData(store.agentDetail(agentId(agent.id)).current);
  const { key } = accountSelection(ui, agent, openRequest);
  const roles = accountsForRoles(
    agent,
    ui.accounts,
    detail ?? {},
    (item) => item.id,
    (item) => ui.projectionFor(item)?.projection.runway,
  );
  return (
    <div
      className="account-limits-dots"
      role="group"
      aria-label="Account weekly allowance"
    >
      {/* One dot per role (Lead, Workers, Review), not one per account. */}
      {roles.map(({ role, account: item, automatic }) => {
        const row = { role, account: item, automatic };
        const projection = ui.projectionFor(item)?.projection.runway?.current;
        const current = role === "Lead",
          provider = providerLabel(accountProvider(item)),
          details = roleDetails(row, ui.projectionFor(item));
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
              aria-expanded={opened && key === item.id}
              onClick={() => {
                ui.selectAccount(item.id, key, openRequest ?? 0);
                onChange(true);
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
  );
});

const LimitsDisclosure = observer(function LimitsDisclosure(
  props: UsageControls,
) {
  const { agent, usageStore: ui, opened, onChange: changeOpened } = props;
  const store = useServerStore();
  const phone = useMediaQuery(layoutQueries.navigationCompact);
  const lowAccount = ui.lowAccount;
  const sessionCost =
    props.visible === false
      ? undefined
      : resourceData(
          store.sessionCost(agentId(agent.rootId || agent.id)).current,
        );
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
  return (
    <UsageDisclosure
      phone={phone}
      opened={opened}
      onChange={changeOpened}
      kind="limits"
      target={limitsButton}
    >
      <UsageLimitsPanel {...props} />
    </UsageDisclosure>
  );
});

const UsageLimitsPanel = observer(function UsageLimitsPanel(
  props: UsageControls,
) {
  const { agent, usageStore: ui, openRequest } = props;
  const api = useServerApi();
  const { account, key, fallbackKey } = accountSelection(
    ui,
    agent,
    openRequest,
  );
  const limits = account ? ui.limitsFor(account) : null,
    refreshing = ui.isRefreshing(key);
  const activeProvider = accountProvider(account);
  const reload = (key: string, force = false) => ui.reload(api.get, key, force);
  const selectAccount = (value: string | null) =>
    ui.selectAccount(value, key, openRequest ?? 0);
  return (
    <section
      className="account-limits-panel"
      aria-label="Account limits details"
    >
      <header className="account-limits-heading">
        <h3>Account limits</h3>
        <Button
          size="compact-xs"
          variant="subtle"
          loading={!limits?.data && refreshing}
          onClick={() => void reload(key, true)}
          leftSection={<RefreshCw size={13} />}
        >
          Refresh
        </Button>
      </header>
      <AccountTilesLayout
        label="Accounts with limits"
        provider={accountProvider(account)}
        providers={ui.providers.map((group) => group.provider)}
        onProvider={(provider) =>
          selectAccount(
            ui.providers.find((group) => group.provider === provider)
              ?.accounts[0]?.account.id ?? null,
          )
        }
      >
        <div className="account-limits-tiles">
          {ui.providers.map(({ provider, accounts }) => (
            <Tabs
              key={provider}
              value={
                activeProvider === provider
                  ? key
                  : (accounts[0]?.account.id ?? null)
              }
              onChange={(value) => {
                if (value) selectAccount(value);
              }}
              variant="pills"
              className="account-limits-provider-panel"
              data-active={activeProvider === provider || undefined}
              aria-hidden={activeProvider !== provider}
              inert={activeProvider !== provider}
            >
              <Tabs.List aria-label="Accounts with limits">
                {accounts.map(({ account: item, index }) => {
                  const bucketState =
                    ui.projectionFor(item)?.projection.buckets?.current;
                  const remaining = bucketState?.remaining ?? null;
                  return (
                    <Tabs.Tab
                      key={item.id}
                      value={item.id}
                      className="setup-account"
                      data-account-key={item.id}
                      data-selected={item.id === key || undefined}
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
                        <small className="account-remaining">Unavailable</small>
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
          ))}
        </div>
      </AccountTilesLayout>
      <div className="account-limits-detail-stack">
        {ui.accounts.map((account, index) => (
          <UsageAccount
            key={account.id}
            {...props}
            account={account}
            index={index}
          />
        ))}
      </div>
      <UsageCosts {...props} />
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
  );
});

const UsageAccount = observer(function UsageAccount(props: UsageAccountProps) {
  const {
    agent,
    usageStore: ui,
    openRequest,
    account: activeAccount,
    index: panelIndex,
  } = props;
  const api = useServerApi();
  const { account, key, fallbackKey } = accountSelection(
    ui,
    agent,
    openRequest,
  );
  const isSelected = activeAccount.id === key;
  const projection = ui.projectionFor(activeAccount)?.projection;
  const name = displayAccountName(activeAccount);
  const selectedAccountName =
    name.toLowerCase() === accountProvider(activeAccount) && activeAccount.email
      ? activeAccount.email
      : name;
  const limits = ui.limitsFor(activeAccount),
    loadingLimits = !limits?.data && ui.isRefreshing(activeAccount.id);
  const buckets = projection?.buckets?.current.buckets ?? [],
    recovery = projection?.agents?.[agent.id]?.recovery;
  const refreshing = ui.isRefreshing(key);
  const reload = (key: string, force = false) => ui.reload(api.get, key, force);
  return (
    <div
      key={activeAccount.id}
      className="account-limits-detail-panel"
      data-active={isSelected || undefined}
      aria-hidden={!isSelected}
      inert={!isSelected}
    >
      <div className="account-limits-selected-name">
        <strong title={selectedAccountName}>{selectedAccountName}</strong>
        {activeAccount.plan && (
          <small className="account-plan">{activeAccount.plan}</small>
        )}
        {!account?.email &&
          (!account?.label || activeAccount.label === activeAccount.id) && (
            <small>{account?.id || fallbackKey}</small>
          )}
      </div>
      {(projection?.status.kind === "checking" ||
        projection?.status.kind === "pending") && (
        <p role="status">Checking account limits…</p>
      )}
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
              {bucket.planType !== null && <span>{bucket.planType}</span>}
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
                      className={window.warning ? "account-limits-warning" : ""}
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
                        <UsageTime at={window.reset} />
                      </>
                    ) : (
                      <span>Reset time unavailable</span>
                    )}
                  </div>
                </div>
              ))}
            </div>
            {!bucket.windows.length && (
              <p className="account-limits-empty">No limit windows reported.</p>
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
      {account?.provider === ProviderKind.Claude && (
        <section
          className="account-reset-credits"
          aria-label="Claude free limit resets"
        >
          <header>
            <strong>Free limit resets</strong>
            <span>Check on Claude</span>
          </header>
          <p>
            Opens Claude. Use the same account, then confirm “Reset for free”.
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
              onClick={() => void reload(key, true)}
            >
              Refresh after reset
            </Button>
          </div>
        </section>
      )}
      <UsageResetCredits {...props} />
    </div>
  );
});

const UsageResetCredits = observer(function UsageResetCredits(
  props: UsageAccountProps,
) {
  const { agent, usageStore: ui, account: activeAccount, openRequest } = props;
  const store = useServerStore(),
    api = useServerApi();
  const { account, key } = accountSelection(ui, agent, openRequest);
  const isSelected = activeAccount.id === key;
  const resetCredits = ui.projectionFor(activeAccount)?.projection.reset;
  if (!resetCredits?.present) return null;
  const { resetPending, resetApplied } = ui;
  const feedback =
    ui.feedback.key === key
      ? ui.feedback
      : { confirm: null, error: "", notice: "" };
  const requestReset = (credit: string | null) => ui.requestReset(key, credit);
  const applyReset = (credit: string) =>
    ui.applyReset(api, store, account, credit);
  const resetButton = (
    label: string,
    onClick: () => void,
    options: Pick<ButtonProps, "variant" | "disabled" | "loading"> = {},
  ) => (
    <Button
      size="compact-xs"
      tabIndex={isSelected ? undefined : -1}
      {...options}
      disabled={resetPending || !!options.disabled}
      onClick={onClick}
    >
      {label}
    </Button>
  );
  return (
    <section className="account-reset-credits" aria-label="Limit reset credits">
      <header>
        <strong>Limit resets</strong>
        <span>{resetCredits.countLabel}</span>
      </header>
      {resetCredits.credits.map((credit) => {
        const applied = resetApplied.has(resetKey(credit.id, activeAccount.id));
        return (
          <div className="account-reset-credit" key={credit.id}>
            <div className="account-reset-credit-row">
              <div>
                <strong title={credit.description ?? undefined}>
                  {credit.title}
                </strong>
                <span>
                  {credit.expiresAt !== null ? (
                    <>
                      Expires <UsageTime at={credit.expiresAt} />
                    </>
                  ) : (
                    "Expiration unavailable"
                  )}
                </span>
              </div>
              {resetButton(
                applied ? "Applied" : "Apply reset",
                () => requestReset(credit.id),
                { variant: "light", disabled: applied || !credit.enabled },
              )}
            </div>
            {isSelected && feedback.confirm === credit.id && (
              <div
                className="account-reset-confirm"
                role="group"
                aria-label={credit.confirmLabel}
              >
                <p>Use this reset credit now? This spends one credit.</p>
                <div>
                  {resetButton("Cancel", () => requestReset(null), {
                    variant: "default",
                  })}
                  {resetButton(
                    "Use one reset credit",
                    () => void applyReset(credit.id),
                    { loading: resetPending, disabled: !credit.enabled },
                  )}
                </div>
              </div>
            )}
          </div>
        );
      })}
      {resetCredits.emptyMessage && <p>{resetCredits.emptyMessage}</p>}
      {isSelected && feedback.error && (
        <p className="account-limits-warning" role="alert">
          {feedback.error}
        </p>
      )}
      {isSelected && feedback.notice && <p role="status">{feedback.notice}</p>}
    </section>
  );
});

export default Usage;
