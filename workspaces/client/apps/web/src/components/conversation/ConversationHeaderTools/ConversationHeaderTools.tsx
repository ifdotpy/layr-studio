import { action } from "mobx";
import { useConversationStore } from "../support/ConversationStore.ts";
import { ObservedNativeError } from "../ObservedNativeError/ObservedNativeError.tsx";
import { observer } from "mobx-react-lite";
import { resolveLayoutLength } from "../../../layout/deriveWorkspaceLayout.ts";
import { Button, Modal, Textarea, Popover, Portal } from "@mantine/core";
import { ListTree } from "lucide-react";
import { currentCapacityRetry } from "../../../capacityRetry.ts";
import { nativeThreadError } from "../../../nativeErrors.ts";
import { ConversationPromptNavigation } from "../ConversationPromptNavigation/ConversationPromptNavigation.tsx";
export function renderConversationHeaderTools() {
  const control = useConversationStore();
  const { actions, opened } = control.context;
  const { layout, navigation, selected, creation, sidebar } = actions;
  const { selectedRoom: room, selectedAgent: agent } = navigation;
  const { detail, planType } = selected;
  const threadBlock = nativeThreadError(agent, detail);
  const capacityRetry = agent ? currentCapacityRetry(agent) : null;
  const accountKey = agent?.accountKey || "default";
  const compactHeaderTools = layout.usesCompactConversationTools;
  const tools = (
    <div className="conversation-prompt-navigation-slot">
      <ConversationPromptNavigation />
    </div>
  );
  const limitRefresh = control.limitRefresh;
  return (
    <>
      <Modal
        opened={!!control.branchDraft}
        onClose={action(() => {
          if (!control.branching) control.branchDraft = null;
        })}
        title={
          control.branchDraft?.before
            ? "Edit in a new chat"
            : "Another answer in a new chat"
        }
      >
        <p>
          Opens a new chat with this message as a draft. Files are not included.
        </p>
        <Textarea
          label="Draft for the new chat"
          value={control.branchDraft?.text || ""}
          autosize
          minRows={3}
          maxRows={10}
          disabled={
            control.branching || !!control.branchDraft?.message.truncated
          }
          onChange={action((event) => {
            const text = event.currentTarget.value;
            control.branchDraft = control.branchDraft
              ? { ...control.branchDraft, text }
              : null;
          })}
        />
        {control.branchDraft?.before &&
          control.branchDraft.message.truncated && (
            <p>
              The preview is clipped. Create the draft to edit the full message.
            </p>
          )}
        <div className="branch-draft-actions">
          <Button
            variant="subtle"
            disabled={control.branching}
            onClick={action(() => (control.branchDraft = null))}
          >
            Cancel
          </Button>
          <Button
            loading={control.branching}
            disabled={!control.branchDraft?.text.trim()}
            onClick={() => {
              if (control.branchDraft)
                void control.branch(
                  control.branchDraft.message,
                  control.branchDraft,
                );
            }}
          >
            Create draft in new chat
          </Button>
        </div>
      </Modal>
      {agent && (detail?.error || threadBlock || capacityRetry) && (
        <ObservedNativeError
          agent={agent}
          detail={detail}
          planType={planType}
          recovery={selected.limitRecovery}
          openLimits={action(() => {
            control.limitsOpen = true;
            control.limitsOpenRequest++;
          })}
          refreshLimits={() => limitRefresh.reload(accountKey, true)}
          newChat={creation.newChat}
          chooseChat={sidebar.showAndFocusSearch}
        />
      )}
      {!layout.usesMenuHeaderTools && !room && (opened || control.managed) && (
        <Portal target="#conversation-header-tools">
          {compactHeaderTools ? (
            <Popover
              opened={control.toolsExpanded}
              onChange={action((value) => {
                control.toolsExpanded = value;
              })}
              position="bottom-start"
              floatingStrategy="fixed"
              withinPortal={false}
              keepMounted
              transitionProps={{ duration: 0 }}
              width="min(var(--layout-tools-menu-max), calc(100vw - 2 * var(--layout-tools-menu-gutter)))"
              offset={resolveLayoutLength(
                "tools.menuGap",
                Number.parseFloat(
                  getComputedStyle(document.documentElement).fontSize,
                ),
              )}
            >
              <details
                className="conversation-header-tools-menu"
                data-compact="yes"
                open={control.toolsExpanded}
              >
                <Popover.Target>
                  <summary
                    className="conversation-header-tools-summary"
                    aria-label="Conversation tools"
                    title="Conversation tools"
                    onClick={action((event) => {
                      event.preventDefault();
                      control.toolsExpanded = !control.toolsExpanded;
                    })}
                  >
                    <ListTree size={18} />
                    <span className="sr-only">Conversation tools</span>
                  </summary>
                </Popover.Target>
                <Popover.Dropdown className="conversation-header-tools-content">
                  {tools}
                </Popover.Dropdown>
              </details>
            </Popover>
          ) : (
            <div className="conversation-header-tools-menu" data-compact="no">
              <div className="conversation-header-tools-content">{tools}</div>
            </div>
          )}
        </Portal>
      )}
    </>
  );
}

export const ConversationHeaderTools = observer(renderConversationHeaderTools);
