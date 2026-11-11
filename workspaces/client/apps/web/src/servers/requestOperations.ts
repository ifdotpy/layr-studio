import { projectLocationOperation } from "./projectLocations.ts";
import { renameOperation } from "../renameRequest.ts";
import { RequestRegistry, type OperationContext } from "./RequestRegistry.ts";

/** Every migrated operation, so a reload can revive its unresolved requests. */
export const requestOperations = [renameOperation, projectLocationOperation];

export function createRequestRegistry(
  storage: Storage,
  api: OperationContext["api"],
  store: OperationContext["store"],
  notify: OperationContext["notify"],
) {
  return new RequestRegistry(
    storage,
    { api, store, notify },
    requestOperations,
  );
}
