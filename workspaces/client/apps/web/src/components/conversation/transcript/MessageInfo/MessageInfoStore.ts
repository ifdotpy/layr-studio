import { makeAutoObservable } from "mobx";
import { derived } from "../../support/derived.ts";
import type { MessageInfoProps } from "./MessageInfo.tsx";
import type { GetOptions, GetResult } from "../../../../api.ts";
import { localDateTime } from "../../../../local-time.ts";
import { resourceData, type ReadSlot } from "../../../../store/runtime.ts";
import { agentId as storeAgentId } from "../../../../generated/observableResources.ts";
import type {
  useServerStore,
  useServerNet,
} from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
export type MessageInfoSource = Pick<
  ReturnType<typeof useServerStore>,
  "sessionCost"
>;
type Owner = MessageInfoSource;
type Query = NonNullable<GetOptions<"/api/analytics">["query"]>;
const knownNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value) && value >= 0;
const tokens = (value: unknown) =>
  knownNumber(value) ? value.toLocaleString() : undefined;
const duration = (value: number) =>
  value === 0
    ? "0 s"
    : value < 1000
      ? `${Math.max(1, Math.round(value))} ms`
      : `${(value / 1000).toLocaleString(undefined, { maximumFractionDigits: 1 })} s`;

export class MessageInfoStore {
  mounted = false;
  selection: {
    owner: Owner;
    slot: ReadSlot<GetResult<"/api/analytics">>;
  } | null = null;
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true, deep: false });
  }
  close() {
    this.selection = null;
  }
  changeOpen(
    next: boolean,
    query: Query | null,
    owner: Owner,
    net: ReturnType<typeof useServerNet>,
  ) {
    if (!next || !query) {
      this.close();
      return;
    }
    this.mounted = true;
    const slot = net.reads.analytics(query);
    this.selection = { owner, slot };
    void slot.read().catch(() => {});
  }
  view = derived(
    (
      message: MessageInfoProps["message"],
      agentId: MessageInfoProps["agentId"],
      rootId: MessageInfoProps["rootId"],
      accounts: MessageInfoProps["accounts"],
      store: Owner,
    ) => {
      const current =
        this.selection?.owner === store
          ? this.selection.slot.current
          : undefined;
      const open = !!current;
      const itemId =
        agentId && message.id.startsWith(`${agentId}:`)
          ? message.id.slice(agentId.length + 1)
          : message.id;
      const metadata = current ? resourceData(current) : undefined;
      const loading = current?.status === "loading",
        failed = current?.status === "error";

      const at =
        message.at ?? message.created ?? message.timestamp ?? metadata?.at;
      const date =
        at == null || at === ""
          ? null
          : new Date(typeof at === "number" ? at * 1000 : at);
      const account = accounts?.find(
        (entry) => entry.id === (metadata?.accountKey || message.accountKey),
      );
      const currentCost =
        open && rootId
          ? resourceData(store.sessionCost(storeAgentId(rootId)).current)
          : undefined;
      const cost = currentCost?.rootId === rootId ? currentCost : null;
      const rows: [string, string, boolean?][] = [];
      const add = (
        label: string,
        value: string | null | undefined,
        copy = false,
      ) => {
        if (value) rows.push(copy ? [label, value, true] : [label, value]);
      };
      if (date && Number.isFinite(date.getTime()) && date.getTime() > 0)
        rows.push([
          "Local date and time",
          localDateTime(date, {
            year: "numeric",
            month: "short",
            day: "numeric",
            hour: "2-digit",
            minute: "2-digit",
            second: "2-digit",
          }),
        ]);
      add("Model", metadata?.model || message.model);
      add("Reasoning effort", metadata?.effort || message.effort);
      add("Provider", metadata?.provider || account?.provider);
      add("Account", metadata?.accountLabel || account?.label);
      add("Run ID", metadata?.runId);
      add("Attempt ID", metadata?.attemptId);
      if (knownNumber(metadata?.turnDurationMs))
        rows.push(["Turn duration", duration(metadata.turnDurationMs)]);
      add("Response output tokens", tokens(metadata?.tokens?.outputTokens));
      add(
        "Response reasoning tokens",
        tokens(metadata?.tokens?.reasoningOutputTokens),
      );
      if (knownNumber(metadata?.responseRate))
        rows.push([
          "Response output rate",
          `${metadata.responseRate.toLocaleString(undefined, { maximumFractionDigits: 1 })} tok/s`,
        ]);
      if (cost?.pricingState === "ready" && knownNumber(cost.totalUSD))
        rows.push([
          "Session estimate (USD)",
          cost.totalUSD > 0 && cost.totalUSD < 0.0001
            ? "< $0.0001"
            : `$${cost.totalUSD.toLocaleString(undefined, { maximumFractionDigits: 4 })}`,
        ]);

      add("Turn ID", message.turnId ?? undefined, true);
      add("Item ID", itemId, true);
      const query: Query | null = agentId
        ? {
            agent: agentId,
            view: "message-info",
            item: itemId,
            ...(message.turnId ? { turn: message.turnId } : {}),
            ...(message.threadId ? { thread: message.threadId } : {}),
          }
        : null;
      return { current, open, loading, failed, rows, query };
    },
  );
}
