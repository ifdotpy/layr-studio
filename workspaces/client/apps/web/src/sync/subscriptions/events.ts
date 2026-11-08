import * as api from "../../api.ts";
/** Internal adapters for projection, recovery and non-React consumers. Components
 * use server resources; transport details stay within the subscription boundary. */
import * as transport from "../resourceEvents";
import { subscribeTranscriptProjection } from "../client";
import { createSubscriptions } from "./module";
import { createDataObserver } from "./data";
import { subscribeEntityData } from "./entityData";

const subscriptions = createSubscriptions(
  transport,
  createDataObserver(
    () => ({ subscribeTranscriptProjection }),
    transport,
    subscribeEntityData,
    undefined,
    api,
  ),
);
export const {
  watchResourceChanges,
  watchResourceConnection,
  watchTokenRateEvents,
  watchLiveDeltas,
  acknowledgeEntitySequences,
} = subscriptions;
export const defaultSubscriptions = subscriptions;
export { createSubscriptions } from "./module";
export { createDataObserver } from "./data";
export type * from "../resourceEvents";
