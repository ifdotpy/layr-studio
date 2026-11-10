/** Name of the projection store of one workspace and API schema. */
export function entityProjectionDatabaseName(
  workspaceId: string,
  schemaHash: string,
  serverSuffix = "",
) {
  return `studio-entity-projection-${workspaceId}${serverSuffix}-${schemaHash}`;
}
