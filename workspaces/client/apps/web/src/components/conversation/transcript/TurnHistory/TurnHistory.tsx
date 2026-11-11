import type { TranscriptStore } from "../../support/TurnHistoryStore.ts";
import { HistoryLiveText } from "../../../../conversation/liveText.ts";
import { observer } from "mobx-react-lite";
import { reportPromptComposerRender } from "../../../../perf/promptComposerRenderProbe.ts";
import HistoryWindow from "../HistoryWindow/HistoryWindow.tsx";

export default observer(function TurnHistory({
  store,
}: {
  store: TranscriptStore;
}) {
  reportPromptComposerRender("turn-history", store.presentation.agentId);
  return (
    <HistoryLiveText.Provider value={store.presentation.live || null}>
      <HistoryWindow model={store} />
    </HistoryLiveText.Provider>
  );
});
