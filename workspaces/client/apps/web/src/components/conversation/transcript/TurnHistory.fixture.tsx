import type { ComponentProps, ReactNode } from "react";
import type { Message } from "../../../types";
import {
  TranscriptProvider,
  type ConversationTranscriptActions,
} from "../navigation";
import TurnHistory from "./TurnHistory";

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
