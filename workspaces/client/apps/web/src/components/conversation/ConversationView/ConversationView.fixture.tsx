import { ConversationStore } from "../support/ConversationStore.ts";
import { createConversationContext } from "../support/conversationContext.ts";
import { useConversationActions } from "../support/navigation.ts";
import type { ServerAreas } from "../../../app/ServerAreasProvider.tsx";
export function mountConversationFixture(areas: ServerAreas) {
  const { selected, navigation, startup } = areas;
  if (
    selected.conversation?.context.opened !== navigation.selectedId ||
    selected.conversation?.context.stateDir !== startup.creationScope ||
    selected.conversation.context.kind !== selected.kind
  ) {
    selected.closeConversation();
    selected.requestOwner = selected.createRequestOwner();
    selected.requestOwner.start();
    selected.conversation = new ConversationStore(
      createConversationContext(areas, selected.attachmentDrafts),
    );
  }
}
import { cloneElement, isValidElement, type ReactNode } from "react";
import { renderConversationView } from "./ConversationView.tsx";
import {
  ConversationHeaderTools,
  renderConversationHeaderTools,
} from "../ConversationHeaderTools/ConversationHeaderTools.tsx";
import {
  ConversationTranscript,
  renderConversationTranscript,
} from "../ConversationTranscript/ConversationTranscript.tsx";
import {
  ConversationFollow,
  renderConversationFollow,
} from "../ConversationFollow/ConversationFollow.tsx";
import {
  ConversationQueueArea,
  renderConversationQueueArea,
} from "../ConversationQueueArea/ConversationQueueArea.tsx";
import {
  ConversationComposerArea,
  renderConversationComposerArea,
} from "../ConversationComposerArea/ConversationComposerArea.tsx";
import {
  ConversationFooter,
  renderConversationFooter,
} from "../ConversationFooter/ConversationFooter.tsx";
const leaves = new Map<unknown, () => ReactNode>([
  [ConversationHeaderTools, renderConversationHeaderTools],
  [ConversationTranscript, renderConversationTranscript],
  [ConversationFollow, renderConversationFollow],
  [ConversationQueueArea, renderConversationQueueArea],
  [ConversationComposerArea, renderConversationComposerArea],
  [ConversationFooter, renderConversationFooter],
]);
/** Fake-hook callers still traverse the real owner-bound leaf implementations. */
export function renderConversationViewFixture() {
  mountConversationFixture(useConversationActions());
  const expand = (node: ReactNode): ReactNode => {
    if (Array.isArray(node)) return node.map(expand);
    if (!isValidElement<{ children?: ReactNode }>(node)) return node;
    const leaf = leaves.get(node.type);
    return leaf
      ? expand(leaf())
      : cloneElement(node, {}, expand(node.props.children));
  };
  return expand(renderConversationView());
}
