import { CircleAlert } from "lucide-react";
import { providerLabel } from "../../generated/providerKind.ts";
import { observer } from "mobx-react-lite";
import { ChatOperationalKind } from "../../generated/api.ts";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import type { ChatIndicator } from "../chat-status/chatStatusModel";
import {
  displayedIndicator,
  type IndicatorRow,
} from "../chat-row/indicator.ts";
import "./chat-status.css";
import "./provider-activity.css";
import { useVisualActivityRef } from "../../hooks/useVisualActivity";

type Props =
  | {
      row: IndicatorRow;
      pendingRead?: SidebarChatRow["markRead"] | undefined;
      model?: string | undefined;
    }
  | {
      status?: ChatIndicator | undefined;
      provider?: string | undefined;
      model?: string | undefined;
    };

export default observer(function ChatStatus(props: Props) {
  const status =
    "row" in props
      ? displayedIndicator(props.row, props.pendingRead)
      : props.status;
  const provider = "row" in props ? props.row.provider : props.provider;
  const model =
    props.model ??
    ("row" in props ? (props.row.model ?? undefined) : undefined);
  const visualRef = useVisualActivityRef<HTMLSpanElement>();
  if (!status) return null;
  const kind = status.kind;
  switch (kind) {
    case ChatOperationalKind.none:
    case ChatOperationalKind.paused:
      return null;
    case ChatOperationalKind.working:
    case ChatOperationalKind.unread:
    case ChatOperationalKind.answer:
    case ChatOperationalKind.error:
      break;
    default:
      return kind satisfies never;
  }
  const modelLabel = model ? `${providerLabel(provider || "")} · ${model}` : "";
  const label = [status?.label, modelLabel].filter(Boolean).join(" · ");
  return (
    <span
      ref={visualRef}
      className={`chat-status chat-status-${kind}`}
      data-chat-status={kind}
      data-agent-provider={provider || ""}
      role="img"
      aria-label={label}
      title={label}
    >
      {kind === "working" ? (
        <svg
          width="13"
          height="13"
          viewBox="0 0 16 16"
          fill="none"
          aria-hidden="true"
        >
          <circle
            cx="8"
            cy="8"
            r="5.5"
            pathLength="100"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeDasharray="76 24"
          />
        </svg>
      ) : kind === "error" ? (
        <CircleAlert size={13} />
      ) : (
        <span className="chat-status-dot" />
      )}
    </span>
  );
});
