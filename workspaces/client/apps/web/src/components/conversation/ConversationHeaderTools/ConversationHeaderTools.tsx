import { action } from "mobx";
import { useConversationStore } from "../support/ConversationStore.ts";
import { ObservedNativeError } from "../ObservedNativeError/ObservedNativeError.tsx";
import { useLimitRefresh } from "../../../usage/useLimitRefresh.ts";
import { observer } from "mobx-react-lite";
import { resolveLayoutLength } from "../../../layout/deriveWorkspaceLayout.ts";
import { Button, Modal, Textarea } from "@mantine/core";
import { createPortal } from "react-dom";
import { ListTree } from "lucide-react";
import { useLayoutEffect, useEffect, useState } from "react";
import { currentCapacityRetry } from "../../../capacityRetry.ts";
import { nativeThreadError } from "../../../nativeErrors.ts";
import { ConversationPromptNavigation } from "../ConversationPromptNavigation/ConversationPromptNavigation.tsx";
export function renderConversationHeaderTools() {
  const control = useConversationStore();
  const { actions, opened, api } = control.context;
  const { layout, navigation, selected, creation, sidebar } = actions;
  const { selectedRoom: room, selectedAgent: agent } = navigation;
  const { detail, planType } = selected;
  const threadBlock = nativeThreadError(agent, detail);
  const capacityRetry = agent ? currentCapacityRetry(agent) : null;
  const accountKey = agent?.accountKey || "default";
  const compactHeaderTools = layout.usesCompactConversationTools;
  const [headerTools, setHeaderTools] = useState<HTMLElement | null>(null);
  const [toolsMenuPosition, setToolsMenuPosition] = useState<{
    left: number;
    top: number;
    width: number;
  } | null>(null);
  useLayoutEffect(() => {
    setHeaderTools(
      layout.usesMenuHeaderTools
        ? null
        : document.getElementById("conversation-header-tools"),
    );
  }, [layout.usesMenuHeaderTools]);
  useEffect(
    action(() => {
      if (!compactHeaderTools || !control.toolsExpanded || !headerTools) return;
      const menu = headerTools.querySelector(".conversation-header-tools-menu");
      if (!menu) return;
      const dismissOutside = action((event: PointerEvent) => {
        const target = event.target;
        if (!(target instanceof Element)) return;
        if (
          menu.contains(target) ||
          target.closest(".mantine-Popover-dropdown")
        )
          return;
        control.toolsExpanded = false;
      });
      const dismissEscape = action((event: KeyboardEvent) => {
        if (event.key === "Escape") control.toolsExpanded = false;
      });
      document.addEventListener("pointerdown", dismissOutside, true);
      document.addEventListener("keydown", dismissEscape, true);
      return () => {
        document.removeEventListener("pointerdown", dismissOutside, true);
        document.removeEventListener("keydown", dismissEscape, true);
      };
    }),
    [compactHeaderTools, headerTools, control.toolsExpanded],
  );
  useLayoutEffect(
    action(() => {
      if (!compactHeaderTools || !control.toolsExpanded || !headerTools) {
        setToolsMenuPosition(null);
        return;
      }
      const summary = headerTools.querySelector(
        ".conversation-header-tools-summary",
      );
      if (!summary) return;
      const updatePosition = () => {
        const anchor = summary.getBoundingClientRect();
        const rootFontPx = Number.parseFloat(
          getComputedStyle(document.documentElement).fontSize,
        );
        const compactToolsMenuMaxWidthPx = resolveLayoutLength(
          "tools.menuMax",
          rootFontPx,
        );
        const compactToolsMenuViewportGutterPx = resolveLayoutLength(
          "tools.menuGutter",
          rootFontPx,
        );
        const compactToolsMenuGapPx = resolveLayoutLength(
          "tools.menuGap",
          rootFontPx,
        );
        const viewportWidth =
          document.documentElement.clientWidth || window.innerWidth;
        const menuWidth = Math.min(
          compactToolsMenuMaxWidthPx,
          Math.max(1, viewportWidth - compactToolsMenuViewportGutterPx * 2),
        );
        const minLeft = compactToolsMenuViewportGutterPx;
        const maxLeft = Math.max(minLeft, viewportWidth - menuWidth - minLeft);
        setToolsMenuPosition({
          left: Math.max(minLeft, Math.min(anchor.left, maxLeft)),
          top: anchor.bottom + compactToolsMenuGapPx,
          width: menuWidth,
        });
      };
      updatePosition();
      const observer = new ResizeObserver(updatePosition);
      observer.observe(summary);
      const workspace = headerTools.closest<HTMLElement>(".workspace");
      if (workspace) observer.observe(workspace);
      window.addEventListener("resize", updatePosition);
      window.visualViewport?.addEventListener("resize", updatePosition);
      return () => {
        observer.disconnect();
        window.removeEventListener("resize", updatePosition);
        window.visualViewport?.removeEventListener("resize", updatePosition);
      };
    }),
    [compactHeaderTools, headerTools, control.toolsExpanded],
  );
  const limitRefresh = useLimitRefresh(api.get);
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
      {!layout.usesMenuHeaderTools &&
        headerTools &&
        !room &&
        (opened || control.managed) &&
        createPortal(
          <details
            className="conversation-header-tools-menu"
            data-compact={compactHeaderTools ? "yes" : "no"}
            open={!compactHeaderTools || control.toolsExpanded}
            onToggle={action((event) => {
              if (compactHeaderTools)
                control.toolsExpanded = event.currentTarget.open;
            })}
          >
            <summary
              className="conversation-header-tools-summary"
              aria-label="Conversation tools"
              title="Conversation tools"
            >
              <ListTree size={18} />
              <span className="sr-only">Conversation tools</span>
            </summary>
            <div
              className="conversation-header-tools-content"
              style={
                compactHeaderTools && control.toolsExpanded && toolsMenuPosition
                  ? {
                      left: toolsMenuPosition.left,
                      top: toolsMenuPosition.top,
                      width: toolsMenuPosition.width,
                    }
                  : compactHeaderTools
                    ? { visibility: "hidden" }
                    : undefined
              }
            >
              {!room && opened && (
                <div className="conversation-prompt-navigation-slot">
                  <ConversationPromptNavigation />
                </div>
              )}
            </div>
          </details>,
          headerTools,
        )}
    </>
  );
}

export const ConversationHeaderTools = observer(renderConversationHeaderTools);
