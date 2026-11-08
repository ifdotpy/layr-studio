import { useReadView } from "../../useReadView.ts";
import { observer } from "mobx-react-lite";
import { useServerApi } from "../../../servers/ServerServicesContext";
import { useRef, useState } from "react";
import { ActionIcon, Popover } from "@mantine/core";
import { Copy, Info, X } from "lucide-react";
import { type GetOptions, type GetResult } from "../../../api";
import { localDateTime } from "../../../local-time";
import { useServerStore } from "../../../servers/ServerServicesContext";
import { resourceData, type ReadSlot } from "../../../store/runtime.ts";
import { agentId as storeAgentId } from "../../../generated/observableResources.ts";
import type { Message } from "../../../types";
import type { Account } from "../../Accounts.tsx";
import "./message-info.css";

export type MessageInfoProps = {
  message: Message;
  agentId?: string | undefined;
  stateDir: string;
  rootId?: string | undefined;
  accounts?:
    | readonly Pick<Account, "id" | "label" | "email" | "provider">[]
    | undefined;
  onCopy: (value: string) => void;
};

const knownNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value) && value >= 0;
const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const duration = (value: number) =>
  value === 0
    ? "0 s"
    : value < 1000
      ? `${Math.max(1, Math.round(value))} ms`
      : `${(value / 1000).toLocaleString(undefined, { maximumFractionDigits: 1 })} s`;

export default observer(function MessageInfo({
  message,
  agentId,
  rootId,
  accounts,
  onCopy,
}: MessageInfoProps) {
  const store = useServerStore();
  const api = useServerApi();

  const [selection, setSelection] =
    useReadView<ReadSlot<GetResult<"/api/analytics">>>();
  const current = selection?.current;
  const open = !!current;
  const [mounted, setMounted] = useState(false);
  const target = useRef<HTMLButtonElement>(null);
  // Transcript items can carry renderer metadata; validate optional extension
  // fields at runtime before displaying or querying with them.
  const details: Record<string, unknown> = isRecord(message) ? message : {};
  const threadId =
    typeof details.threadId === "string" ? details.threadId : undefined;
  const timestamp =
    typeof details.timestamp === "number" ||
    typeof details.timestamp === "string"
      ? details.timestamp
      : undefined;
  const messageAccountKey =
    typeof details.accountKey === "string" ? details.accountKey : undefined;
  const messageModel =
    typeof details.model === "string" ? details.model : undefined;
  const messageEffort =
    typeof details.effort === "string" ? details.effort : undefined;
  const itemId =
    agentId && message.id.startsWith(`${agentId}:`)
      ? message.id.slice(agentId.length + 1)
      : message.id;
  const metadata = current ? resourceData(current) : undefined;
  const loading = current?.status === "loading",
    failed = current?.status === "error";
  const changeOpen = (next: boolean) => {
    if (!next || !agentId) {
      setSelection(null);
      return;
    }
    const query = {
      agent: agentId,
      view: "message-info",
      item: itemId,
      ...(message.turnId ? { turn: message.turnId } : {}),
      ...(threadId ? { thread: threadId } : {}),
    } satisfies NonNullable<GetOptions<"/api/analytics">["query"]>;
    const slot = store.reads.analytics(api, query);
    setSelection(slot);
    void slot.read().catch(() => {});
  };
  const close = () => {
    setSelection(null);
    target.current?.focus({ preventScroll: true });
  };
  const at = message.at ?? message.created ?? timestamp ?? metadata?.at;
  const date =
    at == null || at === ""
      ? null
      : new Date(typeof at === "number" ? at * 1000 : at);
  const account = accounts?.find(
    (entry) => entry.id === (metadata?.accountKey || messageAccountKey),
  );
  const currentCost =
    open && rootId
      ? resourceData(store.sessionCost(storeAgentId(rootId)).current)
      : undefined;
  const cost = currentCost?.rootId === rootId ? currentCost : null;
  const rows: [string, string][] = [];
  const identifiers: Array<[string, string | undefined]> = [
    ["Turn ID", message.turnId ?? undefined],
    ["Item ID", itemId],
  ];
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
  if (metadata?.model || messageModel)
    rows.push(["Model", metadata?.model || messageModel || ""]);
  if (metadata?.effort || messageEffort)
    rows.push(["Reasoning effort", metadata?.effort || messageEffort || ""]);
  if (metadata?.provider || account?.provider)
    rows.push(["Provider", metadata?.provider || account!.provider!]);
  if (metadata?.accountLabel || account?.label)
    rows.push(["Account", metadata?.accountLabel || account!.label]);
  if (metadata?.runId) rows.push(["Run ID", metadata.runId]);
  if (metadata?.attemptId) rows.push(["Attempt ID", metadata.attemptId]);
  if (knownNumber(metadata?.turnDurationMs))
    rows.push(["Turn duration", duration(metadata.turnDurationMs)]);
  if (knownNumber(metadata?.tokens?.outputTokens))
    rows.push([
      "Response output tokens",
      metadata.tokens.outputTokens.toLocaleString(),
    ]);
  if (knownNumber(metadata?.tokens?.reasoningOutputTokens))
    rows.push([
      "Response reasoning tokens",
      metadata.tokens.reasoningOutputTokens.toLocaleString(),
    ]);
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
  // A long transcript mounts no popover per message: the popover mounts on the
  // first opening and stays. The trigger is the plain message action button.
  const trigger = (
    <button
      ref={target}
      type="button"
      className="message-action-button message-info-control"
      aria-label="Message information"
      aria-haspopup="dialog"
      aria-expanded={open}
      onClick={() => {
        setMounted(true);
        changeOpen(!open);
      }}
      onKeyDown={(event) => {
        if (open && event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          close();
        }
      }}
    >
      <Info size={14} />
    </button>
  );
  if (!mounted) return trigger;
  return (
    <Popover
      opened={open}
      onChange={changeOpen}
      onDismiss={close}
      position="bottom-end"
      width={340}
      middlewares={{ size: true, shift: { padding: 8 } }}
      withArrow
      trapFocus
      returnFocus
      shadow="md"
      transitionProps={{ duration: 0 }}
    >
      <Popover.Target>{trigger}</Popover.Target>
      <Popover.Dropdown
        className="message-info-popover"
        role="dialog"
        aria-label="Message information"
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            close();
          }
        }}
      >
        <div className="message-info-heading">
          <strong>Message information</strong>
          <ActionIcon
            size="sm"
            aria-label="Close message information"
            data-autofocus
            onClick={close}
          >
            <X size={14} />
          </ActionIcon>
        </div>
        <dl>
          {rows.map(([label, value]) => (
            <div className="message-info-row" key={label}>
              <dt>{label}</dt>
              <dd>{value}</dd>
            </div>
          ))}
          {identifiers.map(([label, value]) =>
            value ? (
              <div className="message-info-row" key={label}>
                <dt>{label}</dt>
                <dd className="message-info-id">
                  <code>{value}</code>
                  <ActionIcon
                    size="sm"
                    aria-label={`Copy ${label.toLowerCase()}`}
                    onClick={() => onCopy(value)}
                  >
                    <Copy size={14} />
                  </ActionIcon>
                </dd>
              </div>
            ) : null,
          )}
        </dl>
        {loading && <p role="status">Read metadata…</p>}
        {failed && <p role="status">Could not read metadata.</p>}
      </Popover.Dropdown>
    </Popover>
  );
});
