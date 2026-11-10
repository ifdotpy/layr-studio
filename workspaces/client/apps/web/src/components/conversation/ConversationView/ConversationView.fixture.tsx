import { cloneElement, isValidElement, type ReactNode } from "react";
import { ConversationView } from "./ConversationView.tsx";
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
  const expand = (node: ReactNode): ReactNode => {
    if (Array.isArray(node)) return node.map(expand);
    if (!isValidElement<{ children?: ReactNode }>(node)) return node;
    const leaf = leaves.get(node.type);
    return leaf
      ? expand(leaf())
      : cloneElement(node, {}, expand(node.props.children));
  };
  return expand(ConversationView());
}
