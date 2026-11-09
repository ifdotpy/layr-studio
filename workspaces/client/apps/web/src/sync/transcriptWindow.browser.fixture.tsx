import { SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS } from "../generated/sharedValues.ts";
import "../rendererStyles.ts";
import {
  useServerResources,
  ServerViewStoreProvider,
} from "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import React from "react";
import { createRoot, type Root } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { useConversationHistory } from "../conversation/historyFixture.ts";
import {
  useConversationScroll,
  TRANSCRIPT_HISTORY_START_PX,
} from "../components/conversation/support/useConversationScroll.ts";
declare global {
  interface Window {
    historyRoot: Root;
    historyReceipts: (string | undefined)[];
  }
}
const workspaceId = document.documentElement.dataset.workspaceId!;
window.historyReceipts = [];
const History = observer(function History() {
  const resources = useServerResources();
  React.useEffect(
    () =>
      resources.watchResourceChanges(
        {
          kind: "transcript",
          agentId: "open",
          tail: SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS,
        },
        (version) => {
          window.historyReceipts.push(version?.replyKind);
        },
        { waitForBaseline: true },
      ),
    [resources],
  );
  const history = useConversationHistory(
    "open",
    "agent",
    true,
    "fixture",
    workspaceId,
  );
  const scrolling = useConversationScroll("window-check", history.loaded);
  return React.createElement(
    "section",
    null,
    React.createElement(
      "button",
      { onClick: () => void history.older(), disabled: !history.before },
      "Older history",
    ),
    React.createElement(
      "div",
      {
        id: "history-scroll",
        ref: scrolling.scroll,
        style: { height: "100px", overflowY: "auto" },
        onScroll: () => {
          scrolling.onScroll();
          if (
            history.before &&
            !history.pageLoading &&
            scrolling.scroll.current!.scrollTop <=
              TRANSCRIPT_HISTORY_START_PX &&
            scrolling.consumeHistoryScrollInput()
          )
            void history.older();
        },
      },
      React.createElement(
        "div",
        { ref: scrolling.content },
        history.items.map((item) =>
          React.createElement(
            "div",
            {
              key: item.id,
              "data-message": item.id,
              style: { height: "20px" },
            },
            item.text,
          ),
        ),
      ),
    ),
    React.createElement(
      "output",
      { "data-count": history.items.length },
      history.items.map((item) => item.id).join(","),
    ),
  );
});
window.historyRoot = createRoot(document.querySelector("main")!);
window.historyRoot.render(
  React.createElement(
    ServerViewStoreProvider,
    null,
    React.createElement(History),
  ),
);
