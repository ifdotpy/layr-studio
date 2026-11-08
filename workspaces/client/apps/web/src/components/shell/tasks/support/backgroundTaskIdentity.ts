import type { ProcessList } from "../../../../generated/observableResources.ts";
export const responseStopIdentity = (
  target: NonNullable<ProcessList["responseTarget"]>,
) => ({ responseTarget: { ...target } });
