import type { ComponentProps, ReactNode } from "react";
import type { Message } from "../../../../types.ts";
import {
  TranscriptProvider,
  type ConversationTranscriptActions,
} from "../../support/navigation.ts";
import TurnHistory from "./TurnHistory.tsx";

/** Standalone checks provide the same action area as the real conversation root. */
export default function TurnHistoryFixture({
  renderMessage,
  onJump,
  questionCards,
  requestCards,
  ...props
}: ComponentProps<typeof TurnHistory> & {
  renderMessage(message: Message): ReactNode;
  onJump(id: string): void;
  questionCards?: ConversationTranscriptActions["questionCards"];
  requestCards?: ConversationTranscriptActions["requestCards"];
}) {
  return (
    <TranscriptProvider
      value={{
        renderMessage,
        jumpToMessage: onJump,
        ...(questionCards ? { questionCards } : {}),
        ...(requestCards ? { requestCards } : {}),
      }}
    >
      <TurnHistory {...props} />
    </TranscriptProvider>
  );
}
