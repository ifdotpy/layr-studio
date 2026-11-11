import "../../src/rendererStyles.ts";
import React, { useState, useMemo, useRef } from "react";
import { createRoot } from "react-dom/client";
import { createConversationScroll } from "../../src/components/conversation/support/useConversationScroll.ts";
import { TranscriptStore } from "../../src/components/conversation/support/TurnHistoryStore.ts";
import { TranscriptProvider } from "../../src/components/conversation/support/navigation.ts";
import HistoryWindow from "../../src/components/conversation/transcript/HistoryWindow/HistoryWindow.tsx";
import type { Message } from "../../src/types.ts";

const messages: Message[] = Array.from({ length: 80 }, (_, index) => ({
  id: "message-" + index,
  role: "user",
  text: "Message " + index,
}));
const transcript = {
  renderMessage: (message: Message) =>
    React.createElement(
      "p",
      { "data-message": message.id, style: { height: 60, margin: 0 } },
      message.text,
    ),
  jumpToMessage() {},
};

export function mount() {
  createRoot(document.body).render(
    React.createElement(function Harness() {
      const [id, select] = useState("workspace-a:agent:lead");
      const [ready, setReady] = useState(false);
      const [revision, redraw] = useState(0);
      const currentReady = useRef(ready);
      currentReady.current = ready;
      const position = useMemo(
        () =>
          createConversationScroll(
            () => currentReady.current,
            () => true,
            id,
          ),
        [id],
      );
      const model = useMemo(
        () =>
          new TranscriptStore(() => ({
            items: ready ? messages : [],
            enabled: false,
            storageKey: id,
            scrollContainer: position.scroll,
            rememberScroll: position.remember,
            getFollow: position.getFollow,
            hasRecentScrollInput: position.hasRecentScrollInput,
          })),
        [id, ready, position],
      );
      window.fixture = { select, setReady, position, redraw };
      return React.createElement(
        "div",
        {
          id: "scroll",
          "data-revision": revision,
          ref: position.scroll,
          onScroll: position.onScroll,
          style: { height: 300, overflow: "auto" },
        },
        React.createElement(
          "div",
          { ref: position.content },
          React.createElement(
            TranscriptProvider,
            { value: transcript },
            React.createElement(HistoryWindow, { model }),
          ),
        ),
      );
    }),
  );
}

declare global {
  interface Window {
    fixture: {
      select: React.Dispatch<React.SetStateAction<string>>;
      setReady: React.Dispatch<React.SetStateAction<boolean>>;
      position: ReturnType<typeof createConversationScroll>;
      redraw: React.Dispatch<React.SetStateAction<number>>;
    };
    followChanges: number;
    followUnsubscribe: () => void;
    messageMeasurements: number;
  }
}
