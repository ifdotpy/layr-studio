import { describe, expect, it, vi } from "vitest";
import {
  EntitySequenceCheckpoint,
  entitySequenceInvalidationCovered,
  getEntitySequenceCheckpoint,
  getEntitySequenceCheckpointForWorkspace,
  setEntitySequenceProjection,
} from "./entitySequence.ts";

describe("entity sequence checkpoint", () => {
  it("publishes coverage only after contiguous data is persisted", async () => {
    const checkpoint = new EntitySequenceCheckpoint();
    checkpoint.assign(100);
    let finish!: () => void;
    const persistence = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const pending = checkpoint.advanceIfContiguous(
      100,
      102,
      checkpoint.resetVersion,
      100,
      () => persistence,
    );
    expect(checkpoint.covers(102)).toBe(false);
    expect(
      entitySequenceInvalidationCovered(checkpoint.value, 102, false),
    ).toBe(false);
    finish();
    expect(await pending).toBe(true);
    expect(checkpoint.covers(102)).toBe(true);
  });

  it("rejects gaps, stale resets, and persistence finishing after a reset", async () => {
    const checkpoint = new EntitySequenceCheckpoint();
    checkpoint.assign(100);
    const persist = vi.fn(async () => {});
    const version = checkpoint.resetVersion;
    expect(
      await checkpoint.advanceIfContiguous(101, 110, version, 100, persist),
    ).toBe(false);
    expect(
      await checkpoint.advanceIfContiguous(100, 110, version, 99, persist),
    ).toBe(false);
    expect(persist).not.toHaveBeenCalled();
    expect(
      await checkpoint.advanceIfContiguous(100, 110, version, 100, async () => {
        checkpoint.reset();
      }),
    ).toBe(false);
    expect(checkpoint.value).toBe(-1);
    expect(
      await checkpoint.advanceIfContiguous(100, 110, version, 100, persist),
    ).toBe(false);
  });

  it("shares one object across re-acquisition and rejects other workspace or schema", async () => {
    const workspace = "checkpoint-workspace";
    const schema = "checkpoint-schema";
    const checkpoint = setEntitySequenceProjection(workspace, schema);
    checkpoint.assign(950);
    let finish!: () => void;
    const persistence = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const persister = persistence.then(() =>
      checkpoint.advanceIfContiguous(
        950,
        1021,
        checkpoint.resetVersion,
        950,
        async () => {},
      ),
    );

    expect(getEntitySequenceCheckpoint("state:entities:v1", schema)).toBe(
      checkpoint,
    );
    expect(
      getEntitySequenceCheckpointForWorkspace(
        "other-workspace",
        "state:entities:v1",
        schema,
      ),
    ).toBeUndefined();
    expect(
      getEntitySequenceCheckpointForWorkspace(
        workspace,
        "state:entities:v1",
        "other-schema",
      ),
    ).toBeUndefined();
    expect(
      getEntitySequenceCheckpointForWorkspace(
        workspace,
        "transcript:one",
        schema,
      ),
    ).toBeUndefined();
    finish();
    expect(await persister).toBe(true);
    expect(checkpoint.value).toBe(1021);
  });

  it("resets the shared object when the active projection changes workspace or schema", () => {
    const first = setEntitySequenceProjection("workspace-one", "schema-one");
    first.assign(120);
    const second = setEntitySequenceProjection("workspace-two", "schema-one");
    expect(second).toBe(first);
    expect(second.value).toBe(-1);
    second.assign(150);
    const schemaChanged = setEntitySequenceProjection(
      "workspace-two",
      "schema-two",
    );
    expect(schemaChanged).toBe(first);
    expect(schemaChanged.value).toBe(-1);
    expect(() =>
      getEntitySequenceCheckpoint("state:entities:v1", "schema-one"),
    ).toThrow();
  });

  it("tells advance listeners the new sequence and when the sequence space resets", () => {
    const checkpoint = new EntitySequenceCheckpoint();
    const seen: Array<[number, boolean]> = [];
    const stop = checkpoint.onAdvance((sequence, reset) =>
      seen.push([sequence, reset]),
    );
    checkpoint.observeEpoch("one");
    checkpoint.assign(5);
    checkpoint.assignWithinEpoch(3);
    checkpoint.assignWithinEpoch(9);
    checkpoint.observeEpoch("two");
    stop();
    checkpoint.assign(1);
    expect(seen).toEqual([
      [5, false],
      [5, false],
      [9, false],
      [-1, true],
    ]);
  });
});
