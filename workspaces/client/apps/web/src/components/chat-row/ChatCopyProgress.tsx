import { observer } from "mobx-react-lite";
import type { SidebarChatRow } from "../../generated/observableResources.ts";
import LayrCopyProgress from "../sidebar/LayrCopyProgress.tsx";

export const ChatCopyProgress = observer(function ChatCopyProgress({row}: {row: Pick<SidebarChatRow, "copyProgress">}) {
  const progress = row.copyProgress;
  return progress && <LayrCopyProgress progress={{phase: progress.phase, done: progress.done, total: progress.total, unit: progress.unit}} />;
});
