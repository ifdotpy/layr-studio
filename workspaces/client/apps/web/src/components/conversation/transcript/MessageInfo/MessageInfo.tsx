import { ownedView } from "../support/ownedView.tsx";
import { MessageInfoStore } from "./MessageInfoStore.ts";
import { useServerNet } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useRef } from "react";
import { ActionIcon, Popover } from "@mantine/core";
import { Copy, Info, X } from "lucide-react";
import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { Message } from "../../../../types.ts";
import type { Account } from "../../../accounts/support/Accounts.tsx";

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

export default ownedView(
  (_props: MessageInfoProps) => new MessageInfoStore(),
  function MessageInfo({ message, agentId, rootId, accounts, onCopy, model }) {
    const store = useServerStore(),
      net = useServerNet();
    const target = useRef<HTMLButtonElement>(null);
    const { open, loading, failed, rows, query } = model.view(
      message,
      agentId,
      rootId,
      accounts,
      store,
    );
    const changeOpen = (next: boolean) =>
      model.changeOpen(next, query, store, net);
    const close = () => {
      model.close();
      target.current?.focus({ preventScroll: true });
    };
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
    if (!model.mounted) return trigger;
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
            {rows.map(([label, value, copy]) => (
              <div className="message-info-row" key={label}>
                <dt>{label}</dt>
                <dd className={copy ? "message-info-id" : undefined}>
                  {copy ? <code>{value}</code> : value}
                  {copy && (
                    <ActionIcon
                      size="sm"
                      aria-label={`Copy ${label.toLowerCase()}`}
                      onClick={() => onCopy(value)}
                    >
                      <Copy size={14} />
                    </ActionIcon>
                  )}
                </dd>
              </div>
            ))}
          </dl>
          {loading && <p role="status">Read metadata…</p>}
          {failed && <p role="status">Could not read metadata.</p>}
        </Popover.Dropdown>
      </Popover>
    );
  },
);
