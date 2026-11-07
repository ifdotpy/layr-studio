import type { ProcessList } from "../../generated/observableResources";
export const responseStopIdentity = (
  target: NonNullable<ProcessList["responseTarget"]>,
) => ({ responseTarget: { ...target } });
