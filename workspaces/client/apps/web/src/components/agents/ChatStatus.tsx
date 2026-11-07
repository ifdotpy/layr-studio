import { CircleAlert } from "lucide-react";
import { providerLabel } from "../../generated/providerKind.ts";
import { observer } from "mobx-react-lite";
import type { SidebarChatView, SidebarAgent } from "../sidebar/viewModel.ts";
import type { ChatIndicator } from "../../status/operationalStatus.ts";
import { displayedIndicator } from "../chat-row/indicator.ts";
import "./chat-status.css";
import "./provider-activity.css";
import { useVisualActivityRef } from "../../hooks/useVisualActivity";

type Props =
  | {
      row: SidebarChatView;
      pendingRead?: SidebarAgent["markRead"] | undefined;
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
  const provider = "row" in props ? props.row.agent.provider : props.provider;
  const model =
    "row" in props ? (props.row.agent.model ?? undefined) : props.model;
  const visualRef = useVisualActivityRef<HTMLSpanElement>();
  if (!status) return null;
  const kind = status.kind;
  switch (kind) {
    case "none":
    case "paused":
      return null;
    case "working":
    case "unread":
    case "answer":
    case "error":
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
