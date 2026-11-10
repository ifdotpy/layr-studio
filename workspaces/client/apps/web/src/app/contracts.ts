/** Stable server composition; retained owners consume its real references. */
export type { ServerAreas } from "./ServerAreasProvider.tsx";
/** Cross-area contracts; generated resource models remain the server data owners. */
import type { PostResult } from "../api.ts";
import type { StateCollections } from "../generated/observableResources.ts";
import type { ServerServices } from "../servers/serverServices.ts";
import type { ScreenChannel } from "../servers/screenChannel.ts";

export type AgentModel = NonNullable<
  ReturnType<StateCollections["agents"]["get"]>
>;
export type ProjectModel = NonNullable<
  ReturnType<StateCollections["projects"]["get"]>
>;
export type CreatedAgent = PostResult<"/api/leads">;
export type Disposer = () => void;
/** Activation is read from the shell's observable selection source, never mirrored from props. */
export interface ScreenSource {
  readonly active: boolean;
  readonly channel: ScreenChannel | null;
}
export interface AppScope {
  readonly services: ServerServices;
  readonly store: ServerServices["store"];
  readonly screen: ScreenSource;
}
export interface SelectionIntent {
  readonly scope: string;
  readonly intent: number;
}
