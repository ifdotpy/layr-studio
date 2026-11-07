import { createArea } from "../area.ts";
import type { ChatActivity } from "../chat-status/chatActivity.ts";
import type { useChatReadState } from "../useChatReadState.ts";
import type {
  DraftReader,
  DraftSubscription,
  DraftWriter,
} from "../prompt-composer/PromptComposer.tsx";
import type { Attachment } from "../ComposerAttachments.tsx";
import type { DraftVersion } from "../../sync/drafts.ts";
import type { DeliveryMoment } from "../../generated/deliveryMoment.ts";
import type { Message } from "../../types.ts";
import type { QuestionCardModel } from "../questions/questionCardModel.ts";
import type { ReactNode } from "react";
import type { useAccounts } from "../Accounts.tsx";

export type ConversationTranscriptActions = {
  renderMessage(message: Message): ReactNode;
  questionCards?(message: Message): QuestionCardModel | undefined;
  requestCards?: ReadonlyMap<string, ReactNode>;
  jumpToMessage(id: string): void;
};

export type ConversationSendOptions = {
  asTextFile?: boolean;
  assets?: string[];
  attachments?: Attachment[];
  onPersist?: () => void | Promise<void>;
  delivery?: DeliveryMoment;
};

export type ConversationActions = {
  accounts: ReturnType<typeof useAccounts>;
  transcript?: ConversationTranscriptActions;
  open(id: string): void;
  getDraft: DraftReader;
  subscribeDraft: DraftSubscription;
  setDraft: DraftWriter;
  send(options?: ConversationSendOptions): Promise<void>;
  refresh(): Promise<void>;
  notify(message: string): void;
  dismissDraft(version: DraftVersion): void;
  observeSends(ids: string[]): void;
  observeRead: ReturnType<typeof useChatReadState>["observeRead"];
  editSend(id: string, text: string): void;
  branchCreated(id: string): void;
  newConversation(): void;
  chooseConversation(): void;
  reloadLimits(): void;
  nativeAction(
    agent: import("../sidebar/viewModel.ts").SidebarAgent,
    action: "compact" | "review",
  ): void;
  openActivity(activity: ChatActivity): void;
  read: Pick<
    ReturnType<typeof useChatReadState>,
    "marking" | "markSidebarUnread" | "pendingReadTargets"
  >;
};
export const [ConversationProvider, useConversationActions] =
  createArea<ConversationActions>("Conversation");
