import { makeAutoObservable, observableRef, runInAction } from "mobx";
import {
  createWorkMirror,
  createAgentDetailMirror,
} from "../../src/generated/observableResources.ts";
import type { SubscriptionState } from "../../src/sync/subscriptions/contracts.ts";
import type { components } from "../../src/generated/api.ts";
export type TaskReviewFrames = {
  [Kind in "agentDetail" | "work"]: SubscriptionState<
    components["schemas"]["SubscriptionDataMap"][Kind]
  >;
};
declare global {
  interface Window {
    taskReviewFrames: TaskReviewFrames;
  }
}
const initial = window.taskReviewFrames;
const work = createWorkMirror(initial.work.data!);
const detail = createAgentDetailMirror(initial.agentDetail.data!);
const workSlot = makeAutoObservable(
  { current: { status: "live" as const, data: work.current } },
  { current: observableRef },
);
const detailSlot = makeAutoObservable(
  { current: { status: "live" as const, data: detail.current } },
  { current: observableRef },
);
window.addEventListener("task-review-frame", () =>
  runInAction(() => {
    const frame = window.taskReviewFrames;
    if (frame.work.data) work.apply(frame.work.data);
    if (frame.agentDetail.data) detail.apply(frame.agentDetail.data);
  }),
);
const store = { work: () => workSlot, agentDetail: () => detailSlot };
export const useServerStore = () => store;
