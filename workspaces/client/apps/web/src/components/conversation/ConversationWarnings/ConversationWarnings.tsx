import { useState } from "react";
import { observer } from "mobx-react-lite";
import { warningTitle, isInfoNotice } from "../../../app/notices/warnings.ts";
import { ActionIcon, Badge, Modal, Tabs, Text } from "@mantine/core";
import { CircleAlert, Info } from "lucide-react";
import { errorDetails } from "../../../errorPresentation.ts";
import ErrorDescription from "../../shell/ErrorDescription/ErrorDescription.tsx";

import {
  LEVEL_COLORS,
  StudioErrorList,
  useStudioErrors,
} from "../../shell/StudioErrorList/StudioErrorList.tsx";

export default observer(function ConversationWarnings() {
  const errors = useStudioErrors();
  const {
    logs,
    accountNotices,
    chatWarnings,
    highestLevel: highest,
    hasWarning,
    noticeCount,
    warningTitle: title,
    warningCount: count,
  } = errors;
  const scope = errors.areas.selected.warningScope;
  const [openedScope, setOpenedScope] = useState<string | null>(null);
  const close = () => setOpenedScope(null);
  return (
    <>
      <ActionIcon
        className={`conversation-warning-trigger${hasWarning ? "" : " conversation-info-trigger"}`}
        variant="subtle"
        color={hasWarning && highest ? LEVEL_COLORS[highest] : "gray"}
        aria-label={title}
        title={`${title} (${count})`}
        onClick={() => {
          setOpenedScope(scope);
        }}
      >
        {hasWarning ? (
          <CircleAlert size={18} aria-hidden="true" />
        ) : (
          <Info size={18} aria-hidden="true" />
        )}
        {count > 0 && (
          <Badge
            className="conversation-warning-count"
            size="xs"
            aria-label={`${count} notices and app errors`}
          >
            {count}
          </Badge>
        )}
      </ActionIcon>
      <Modal
        key={scope}
        opened={openedScope === scope}
        onClose={close}
        title={title}
        size="lg"
        centered
        className="conversation-warnings-modal"
      >
        <Tabs
          defaultValue={noticeCount ? "notices" : "errors"}
          keepMounted={false}
        >
          <Tabs.List>
            <Tabs.Tab value="notices">
              Notices{noticeCount > 0 && ` (${noticeCount})`}
            </Tabs.Tab>
            <Tabs.Tab value="errors">
              App errors
              {errors.items.length + logs.length > 0 &&
                ` (${errors.items.length + logs.length})`}
            </Tabs.Tab>
          </Tabs.List>
          <Tabs.Panel value="notices" pt="md">
            <div className="conversation-warning-list">
              {noticeCount === 0 && <Text size="sm">No notices.</Text>}
              {accountNotices.map((notice) => (
                <article key={`account:${notice.id}`}>
                  <h3>
                    {isInfoNotice(notice)
                      ? notice.id.startsWith("provider-update:")
                        ? "Provider update available"
                        : "Information"
                      : warningTitle(notice.message, "Account notice")}
                  </h3>
                  <p>
                    <ErrorDescription value={notice.message} role="status" />
                  </p>
                  {"details" in notice && notice.details != null && (
                    <details>
                      <summary>Details</summary>
                      <pre>{errorDetails(notice.details)}</pre>
                    </details>
                  )}
                </article>
              ))}
              {chatWarnings.map((message) => {
                return (
                  <article key={`chat:${message.id}`}>
                    <h3>
                      {warningTitle(message.text, "Conversation warning")}
                    </h3>
                    <p>
                      <ErrorDescription value={message.text} role="status" />
                    </p>
                    {"details" in message && message.details != null && (
                      <details>
                        <summary>Details</summary>
                        <pre>{errorDetails(message.details)}</pre>
                      </details>
                    )}
                  </article>
                );
              })}
            </div>
          </Tabs.Panel>
          <Tabs.Panel value="errors" pt="md">
            <StudioErrorList onOpen={close} />
          </Tabs.Panel>
        </Tabs>
      </Modal>
    </>
  );
});
