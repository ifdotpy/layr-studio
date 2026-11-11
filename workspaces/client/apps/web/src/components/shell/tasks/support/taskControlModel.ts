import {
  BackgroundTaskAction,
  BackgroundTaskKind,
  ProcessStdinState,
} from "../../../../generated/api.ts";
import type { TaskProps } from "../TaskStatus/TaskStatus.tsx";
export function restartPatterns(text: string): string[] {
  return text.split("\n").filter(Boolean);
}
export function inputControls(task: TaskProps["task"], closed: boolean) {
  const input = task.display.actions.includes(BackgroundTaskAction.input);
  const stdinClosed =
    closed || task.display.stdinState === ProcessStdinState.closed;
  return {
    native: task.display.kind === BackgroundTaskKind.command,
    stdinClosed,
    unavailable: stdinClosed || !input,
    receiptOnly:
      !input && !task.display.actions.includes(BackgroundTaskAction.resize),
  };
}
export const receiptText = {
  acknowledged: "The process acknowledged the input.",
  rejected: "The process rejected the input.",
  none: "No input request needs a receipt check.",
  unknown: "The input result is unknown. Do not send different input.",
};
