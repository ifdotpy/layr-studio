import { SYNC_ENTITY_SCOPE } from "../generated/api.ts";

const ENTITY_SCOPE = SYNC_ENTITY_SCOPE;

function isContiguous(after: number, checkpoint: number, through: number) {
  return (
    Number.isSafeInteger(after) &&
    Number.isSafeInteger(checkpoint) &&
    Number.isSafeInteger(through) &&
    after <= checkpoint &&
    checkpoint < through
  );
}

/** The in-memory checkpoint shared by this tab's mutation and pull paths. */
export class EntitySequenceCheckpoint {
  private sequence = -1;
  private epoch: string | undefined;
  private resetGeneration = 0;
  private hasAssignment = false;
  private durableCheckpointValid = true;
  private workspaceId: string | undefined;
  private readonly advanceListeners = new Set<
    (sequence: number, reset: boolean) => void
  >();

  get value() {
    return this.sequence;
  }

  get resetVersion() {
    return this.resetGeneration;
  }

  get initialized() {
    return this.hasAssignment;
  }

  get canUseDurableCheckpoint() {
    return this.durableCheckpointValid;
  }

  isSameResetVersion(version: number): boolean {
    return this.resetGeneration === version;
  }

  observeWorkspace(workspaceId: string): boolean {
    if (this.workspaceId === undefined) {
      this.workspaceId = workspaceId;
      return false;
    }
    if (this.workspaceId === workspaceId) return false;
    this.workspaceId = workspaceId;
    this.epoch = undefined;
    this.sequence = -1;
    this.hasAssignment = true;
    this.durableCheckpointValid = false;
    this.resetGeneration++;
    this.notifyAdvance(true);
    return true;
  }

  observeEpoch(epoch: string): boolean {
    if (this.epoch === undefined) {
      this.epoch = epoch;
      return false;
    }
    if (this.epoch === epoch) return false;
    this.epoch = epoch;
    this.sequence = -1;
    this.hasAssignment = true;
    this.durableCheckpointValid = false;
    this.resetGeneration++;
    this.notifyAdvance(true);
    return true;
  }

  assign(sequence: number): void {
    this.sequence = sequence;
    this.hasAssignment = true;
    this.durableCheckpointValid = true;
    this.notifyAdvance();
  }

  /** Preserve a later ack when an older same-epoch pull completes afterward. */
  assignWithinEpoch(sequence: number): void {
    this.sequence = Math.max(this.sequence, sequence);
    this.hasAssignment = true;
    this.notifyAdvance();
  }

  /**
   * Call `listener` with the checkpoint after each assignment. `reset` is true when the
   * sequence space changed (another workspace or epoch), so earlier sequences do not count.
   */
  onAdvance(listener: (sequence: number, reset: boolean) => void): () => void {
    this.advanceListeners.add(listener);
    return () => this.advanceListeners.delete(listener);
  }

  private notifyAdvance(reset = false): void {
    for (const listener of this.advanceListeners)
      listener(this.sequence, reset);
  }

  markDurableCheckpointValid(): void {
    this.durableCheckpointValid = true;
  }

  reset(): void {
    this.sequence = -1;
    this.resetGeneration++;
    this.notifyAdvance(true);
    this.hasAssignment = true;
    this.durableCheckpointValid = false;
  }

  covers(sequence: number): boolean {
    return Number.isSafeInteger(sequence) && sequence <= this.sequence;
  }

  async advanceIfContiguous(
    after: number,
    through: number,
    expectedResetVersion: number,
    durableCheckpoint: number,
    persist: () => Promise<void>,
  ): Promise<boolean> {
    if (
      expectedResetVersion !== this.resetGeneration ||
      !isContiguous(after, this.sequence, through) ||
      !isContiguous(after, durableCheckpoint, through)
    )
      return false;
    await persist();
    if (
      expectedResetVersion !== this.resetGeneration ||
      !isContiguous(after, this.sequence, through)
    )
      return false;
    this.assignWithinEpoch(through);
    return true;
  }
}

/** Checkpoints from different servers never share sequence coverage. */
export function createEntitySequenceScope() {
  const entitySequenceCheckpoint = new EntitySequenceCheckpoint();
  let openWorkspaceId: string | undefined;
  let openSchemaHash: string | undefined;

  /** Reset the shared object when the open projection database changes identity. */
  function setEntitySequenceProjection(
    workspaceId: string,
    schemaHash: string,
  ): EntitySequenceCheckpoint {
    if (openSchemaHash !== undefined && openSchemaHash !== schemaHash) {
      openWorkspaceId = undefined;
      entitySequenceCheckpoint.reset();
    }
    openSchemaHash = schemaHash;
    openWorkspaceId = workspaceId;
    entitySequenceCheckpoint.observeWorkspace(workspaceId);
    return entitySequenceCheckpoint;
  }

  /** Look up the active projection without letting a mutation change its identity. */
  function getEntitySequenceCheckpoint(
    scope: string,
    schemaHash: string,
  ): EntitySequenceCheckpoint {
    if (
      scope !== ENTITY_SCOPE ||
      openWorkspaceId === undefined ||
      schemaHash !== openSchemaHash
    )
      throw new Error("Entity projection checkpoint is not open");
    return entitySequenceCheckpoint;
  }

  function getEntitySequenceCheckpointForWorkspace(
    workspaceId: string,
    scope: string,
    schemaHash: string,
  ): EntitySequenceCheckpoint | undefined {
    if (
      scope !== ENTITY_SCOPE ||
      workspaceId !== openWorkspaceId ||
      schemaHash !== openSchemaHash
    )
      return undefined;
    return getEntitySequenceCheckpoint(scope, schemaHash);
  }

  return {
    setEntitySequenceProjection,
    getEntitySequenceCheckpoint,
    getEntitySequenceCheckpointForWorkspace,
    onEntitySequenceAdvance: (
      listener: (sequence: number, reset: boolean) => void,
    ) => entitySequenceCheckpoint.onAdvance(listener),
  };
}
export const {
  setEntitySequenceProjection,
  getEntitySequenceCheckpoint,
  getEntitySequenceCheckpointForWorkspace,
  onEntitySequenceAdvance,
} = createEntitySequenceScope();

export function entitySequenceInvalidationCovered(
  checkpoint: number,
  requiredSequence: number | undefined,
  unversionedInvalidation: boolean,
): boolean {
  return (
    !unversionedInvalidation &&
    requiredSequence !== undefined &&
    Number.isSafeInteger(checkpoint) &&
    Number.isSafeInteger(requiredSequence) &&
    checkpoint >= requiredSequence
  );
}
