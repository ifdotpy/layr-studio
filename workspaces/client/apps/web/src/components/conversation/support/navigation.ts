import { createArea } from "../../../app/area.ts";
import type { Attachment } from "../ComposerAttachments/ComposerAttachments.tsx";
import type { DeliveryMoment } from "../../../generated/deliveryMoment.ts";
import type { Message } from "../../../types.ts";
import type { QuestionCardModel } from "./questionCardModel.ts";
import type { ReactNode } from "react";

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

export type ConversationActions =
  import("../../../app/ServerAreasProvider.tsx").ServerAreas;
export {
  ServerAreasContextProvider as ConversationProvider,
  useServerAreas as useConversationActions,
} from "../../../app/ServerAreasProvider.tsx";

export const [TranscriptProvider, useTranscriptActions] =
  createArea<ConversationTranscriptActions>("Transcript");
