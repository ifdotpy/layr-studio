import { action, observable } from "mobx";
import { useLayoutEffect, type ReactNode } from "react";
import { Owned } from "../../../ui/Owned/Owned.tsx";
import type { Message } from "../../../../types.ts";
import type { createLiveText } from "../../../../conversation/liveText.ts";
import {
  useServerNet,
  useServerStore,
} from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  TranscriptProvider,
  useTranscriptActions,
  type ConversationTranscriptActions,
} from "../../support/navigation.ts";
import TurnHistory from "./TurnHistory.tsx";
import {
  TranscriptStore,
  type TranscriptSource,
} from "../../support/TurnHistoryStore.ts";

type Props = Omit<
  TranscriptSource,
  "server" | "net" | "live" | "requestCards"
> & {
  history?:
    | { readonly live: ReturnType<typeof createLiveText> | null }
    | undefined;
  renderMessage?: (message: Message) => ReactNode;
  onJump?: (id: string) => void;
  questionCards?: ConversationTranscriptActions["questionCards"];
  requestCards?: ConversationTranscriptActions["requestCards"];
};

/** Test callers supply source snapshots; the application uses its conversation owner. */
export default function TurnHistoryFixture(props: Props) {
  if (!props.renderMessage || !props.onJump)
    return <FixtureSource {...props} />;
  return (
    <TranscriptProvider
      value={{
        renderMessage: props.renderMessage,
        jumpToMessage: props.onJump,
        ...(props.questionCards ? { questionCards: props.questionCards } : {}),
        ...(props.requestCards ? { requestCards: props.requestCards } : {}),
      }}
    >
      <FixtureSource {...props} />
    </TranscriptProvider>
  );
}
function FixtureSource(props: Props) {
  const actions = useTranscriptActions();
  const source: TranscriptSource = {
    ...props,
    server: useServerStore(),
    net: useServerNet(),
    live: props.history?.live,
    requestCards: actions.requestCards,
  };
  return (
    <Owned
      create={() => {
        const input = observable.box(source, { deep: false });
        return { input, model: new TranscriptStore(() => input.get()) };
      }}
    >
      {(owner) => <FixtureSnapshot source={source} owner={owner} />}
    </Owned>
  );
}
function FixtureSnapshot({
  source,
  owner,
}: {
  source: TranscriptSource;
  owner: {
    input: ReturnType<typeof observable.box<TranscriptSource>>;
    model: TranscriptStore;
  };
}) {
  // Test-only source updates stand in for the real observable conversation owner.
  useLayoutEffect(
    action(() => owner.input.set(source)),
    [owner, source],
  );
  return <TurnHistory store={owner.model} />;
}
