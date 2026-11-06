import { Loader } from "@mantine/core";
import { Copy, GitBranch, Pencil, Quote, RotateCcw } from "lucide-react";
import type { ReactNode } from "react";

import MessageInfo, { type MessageInfoProps } from "./MessageInfo";

export type MessageActionsProps = {
  hidden?: boolean;
  info?: MessageInfoProps | undefined;
  onCopy: () => void;
  onQuote?: (() => void) | undefined;
  onEdit?: (() => void) | undefined;
  editLoading?: boolean;
  editDisabled?: boolean;
  onBranch?: (() => void) | undefined;
  branchDisabled?: boolean;
  onAnotherAnswer?: (() => void) | undefined;
  anotherAnswerDisabled?: boolean;
};

/**
 * One message action with a hover hint. The button and hint are plain HTML
 * and CSS (message-info.css), so a transcript re-render costs no Mantine
 * component or tooltip state per action. A disabled action stays focusable
 * through its wrapper and keeps its name.
 */
function Action({
  tip,
  label,
  disabled = false,
  className,
  onClick,
  onPointerDown,
  children,
}: {
  tip: string;
  label: string;
  disabled?: boolean;
  className?: string;
  onClick: () => void;
  onPointerDown?: (event: React.PointerEvent) => void;
  children: ReactNode;
}) {
  return (
    <span
      className="message-action"
      data-tip={tip}
      role={disabled ? "group" : undefined}
      aria-label={disabled ? label : undefined}
      tabIndex={disabled ? 0 : undefined}
    >
      <button
        type="button"
        className={`message-action-button${className ? ` ${className}` : ""}`}
        aria-label={label}
        disabled={disabled}
        onPointerDown={onPointerDown}
        onClick={onClick}
      >
        {children}
      </button>
    </span>
  );
}

export default function MessageActions({
  hidden = false,
  info,
  onCopy,
  onQuote,
  onEdit,
  editLoading = false,
  editDisabled = false,
  onBranch,
  branchDisabled = false,
  onAnotherAnswer,
  anotherAnswerDisabled = false,
}: MessageActionsProps) {
  return (
    <div className="message-bottom" hidden={hidden && !info}>
      <div className="message-action-controls" hidden={hidden}>
        <Action
          tip="Copy this message"
          label="Copy message"
          className="copy-message"
          onClick={onCopy}
        >
          <Copy size={14} />
        </Action>
        {onQuote && (
          <Action
            tip="Quote this message in your reply"
            label="Quote message"
            onPointerDown={(event) => event.preventDefault()}
            onClick={onQuote}
          >
            <Quote size={14} />
          </Action>
        )}
        {onEdit && (
          <Action
            tip="Edit this message in a new chat"
            label="Edit in a new chat"
            disabled={editDisabled}
            onClick={onEdit}
          >
            {editLoading ? <Loader size={14} /> : <Pencil size={14} />}
          </Action>
        )}
        {onBranch && (
          <Action
            tip="Start a new branch after this turn"
            label="Branch after this turn"
            disabled={branchDisabled}
            onClick={onBranch}
          >
            <GitBranch size={14} />
          </Action>
        )}
        {onAnotherAnswer && (
          <Action
            tip="Prepare a request for another answer in a new chat"
            label="Another answer in a new chat"
            disabled={anotherAnswerDisabled}
            onClick={onAnotherAnswer}
          >
            <RotateCcw size={14} />
          </Action>
        )}
      </div>
      {info && <MessageInfo {...info} />}
    </div>
  );
}
