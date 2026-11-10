import * as api from "../../servers/net/api.ts";
/** Internal adapters for projection, recovery and non-React consumers. Components
 * use server resources; transport details stay within the subscription boundary. */
import * as transport from "../resourceEvents.ts";
import { subscribeTranscriptProjection } from "../client.ts";
import { createSubscriptions } from "./module.ts";
import { createDataObserver } from "./data.ts";
import { subscribeEntityData } from "./entityData.ts";

const subscriptions = createSubscriptions(
  transport,
  createDataObserver(
    () => ({ subscribeTranscriptProjection }),
    transport,
    subscribeEntityData,
    api,
  ),
);
export const {
  watchResourceChanges,
  watchResourceConnection,
  watchTokenRateEvents,
  watchLiveDeltas,
  replaceStream,
} = subscriptions;
export const defaultSubscriptions = subscriptions;
export { createSubscriptions } from "./module.ts";
export { createDataObserver } from "./data.ts";
export type * from "../resourceEvents.ts";
