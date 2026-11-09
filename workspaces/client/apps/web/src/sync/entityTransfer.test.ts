import { describe, expect, it } from "vitest";
import {
  EntityTransferAssembler,
  type TransferStorage,
  type StagedEntityInterval,
} from "./entityTransfer.ts";
import type { components } from "../generated/api.ts";
import { SYNC_STREAM_EVENT_MAX_BYTES } from "../generated/sharedValues.ts";

type Batch = components["schemas"]["EntityChangeBatch"];
class MemoryStaging implements TransferStorage {
  readonly rows = new Map<string, string>();
  closed = false;
  async put(key: string, value: string) {
    this.rows.set(key, value);
  }
  async putMany(values: [string, string][]) {
    for (const [key, value] of values) this.rows.set(key, value);
  }
  async get(key: string) {
    return this.rows.get(key);
  }
  async remove(prefix: string) {
    for (const key of this.rows.keys())
      if (key.startsWith(prefix)) this.rows.delete(key);
  }
  async close() {
    this.closed = true;
    this.rows.clear();
  }
}
function batch(after: number, count: number, text = "fixture"): Batch {
  return {
    after,
    through: after + count,
    documents: Array.from({ length: count }, (_, index) => ({
      op: "upsert",
      collection: "workspace",
      id: String(after + index + 1),
      seq: after + index + 1,
      value: { stateDir: text },
    })),
  };
}
function parts(pages: Batch[], scope = "one", fragmentSize = 30000) {
  return pages.flatMap((page, pageIndex) => {
    const data = JSON.stringify(page);
    const partCount = Math.ceil(data.length / fragmentSize);
    return Array.from({ length: partCount }, (_, partIndex) => ({
      protocol: 4,
      workspaceId: "a".repeat(32),
      epoch: "epoch",
      revision: 1,
      reason: "initial",
      resource: { kind: "agentDetail", agentId: scope },
      cursor: { workspaceId: "a".repeat(32), seq: pages.at(-1)!.through },
      replyKind: "snapshot",
      transferId: scope,
      pageIndex,
      payloadKind: "entity",
      partIndex,
      partCount,
      complete: pageIndex + 1 === pages.length && partIndex + 1 === partCount,
      data: data.slice(
        partIndex * fragmentSize,
        (partIndex + 1) * fragmentSize,
      ),
    }));
  });
}
async function pages(interval: StagedEntityInterval | undefined) {
  const result: Batch[] = [];
  if (interval) for await (const page of interval.pages()) result.push(page);
  return result;
}
describe("complete bounded entity transfer", () => {
  it("assembles the default stream transfer without browser database storage", async () => {
    const assembler = new EntityTransferAssembler();
    const complete = await assembler.receive(parts([batch(0, 1)])[0]);
    expect(await pages(complete)).toEqual([batch(0, 1)]);
    await complete?.dispose();
  });
  it("leaves completed staging with its interval owner until disposal", async () => {
    const storage = new MemoryStaging();
    const assembler = new EntityTransferAssembler(storage);
    const removed = await assembler.receive(parts([batch(0, 1)], "removed")[0]);
    const kept = await assembler.receive(parts([batch(0, 1)], "kept")[0]);
    await assembler.remove([{ kind: "agentDetail", agentId: "removed" }]);
    expect(await pages(removed)).toEqual([batch(0, 1)]);
    await removed?.dispose();
    expect(
      [...storage.rows.keys()].some((key) => key.startsWith("removed:")),
    ).toBe(false);
    expect(await pages(kept)).toEqual([batch(0, 1)]);
    expect(storage.closed).toBe(false);
  });
  it("keeps >512 records as separate canonical pages and does not publish a missing final part", async () => {
    const assembler = new EntityTransferAssembler(new MemoryStaging());
    const frames = parts([batch(0, 512), batch(512, 2)]);
    for (const frame of frames.slice(0, -1))
      expect(await assembler.receive(frame)).toBeUndefined();
    const complete = await assembler.receive(frames.at(-1)!);
    expect(
      (await pages(complete)).map((page) => page.documents.length),
    ).toEqual([512, 2]);
    expect(complete?.cursor.seq).toBe(514);
  });
  it("preserves one oversized canonical record including UTF-8 content", async () => {
    const assembler = new EntityTransferAssembler(new MemoryStaging());
    const text = "界".repeat(100000);
    let complete;
    for (const frame of parts([batch(0, 1, text)]))
      complete = await assembler.receive(frame);
    expect((await pages(complete))[0]?.documents[0]).toMatchObject({
      value: { stateDir: text },
    });
  });
  it("accepts duplicate parts, isolates interleaved scopes, and rejects changed replay", async () => {
    const assembler = new EntityTransferAssembler(new MemoryStaging());
    const first = parts([batch(0, 2)], "one", 100);
    const second = parts([batch(0, 2)], "two", 100);
    expect(await assembler.receive(first[0]!)).toBeUndefined();
    expect(await assembler.receive(first[0]!)).toBeUndefined();
    expect(await assembler.receive(second[0]!)).toBeUndefined();
    let one, two;
    for (const frame of first.slice(1)) one = await assembler.receive(frame);
    for (const frame of second.slice(1)) two = await assembler.receive(frame);
    expect(one?.resource).toEqual({ kind: "agentDetail", agentId: "one" });
    expect(two?.resource).toEqual({ kind: "agentDetail", agentId: "two" });
    expect(await assembler.receive(first.at(-1)!)).toBeUndefined();
    await expect(
      assembler.receive({ ...first[0], data: "changed" }),
    ).rejects.toThrow();
  });
  it("cancels incomplete buffers and rejects out-of-order, invalid interval and oversized frames", async () => {
    const storage = new MemoryStaging();
    const assembler = new EntityTransferAssembler(storage);
    const frames = parts([batch(0, 1)], "one", 50);
    await expect(assembler.receive(frames[1]!)).rejects.toThrow();
    await assembler.receive(frames[0]!);
    await assembler.invalidate();
    expect(storage.closed).toBe(true);
    expect(storage.rows.size).toBe(0);
    await expect(assembler.receive(frames[1]!)).rejects.toThrow();
    await expect(
      assembler.receive({
        ...frames[0],
        data: "x".repeat(SYNC_STREAM_EVENT_MAX_BYTES),
      }),
    ).rejects.toThrow();
    await expect(
      new EntityTransferAssembler(new MemoryStaging()).receive(
        parts([{ ...batch(0, 1), through: 0 }])[0]!,
      ),
    ).rejects.toThrow();
  });
  it("accepts documents of one change that share its number and rejects a decrease", async () => {
    const shared: Batch = {
      after: 0,
      through: 2,
      documents: [
        {
          op: "upsert",
          collection: "workspace",
          id: "a",
          seq: 2,
          value: { stateDir: "a" },
        },
        {
          op: "upsert",
          collection: "workspace",
          id: "b",
          seq: 2,
          value: { stateDir: "b" },
        },
      ],
    };
    const complete = await new EntityTransferAssembler(
      new MemoryStaging(),
    ).receive(parts([shared])[0]!);
    expect((await pages(complete))[0]?.documents.map((row) => row.id)).toEqual([
      "a",
      "b",
    ]);
    const decreasing: Batch = {
      ...shared,
      documents: [...shared.documents]
        .reverse()
        .map((row, index) => ({ ...row, seq: 2 - index })),
    };
    await expect(
      new EntityTransferAssembler(new MemoryStaging()).receive(
        parts([decreasing])[0]!,
      ),
    ).rejects.toThrow();
    const atAfter: Batch = {
      after: 2,
      through: 3,
      documents: [{ ...shared.documents[0]!, seq: 2 }],
    };
    await expect(
      new EntityTransferAssembler(new MemoryStaging()).receive(
        parts([atAfter])[0]!,
      ),
    ).rejects.toThrow();
  });
  it("preserves the single-row delta interval without a history snapshot", async () => {
    const assembler = new EntityTransferAssembler(new MemoryStaging());
    const [frame] = parts([batch(513, 1)]);
    const complete = await assembler.receive({
      ...frame,
      replyKind: "delta",
      reason: "change",
    });
    expect((await pages(complete))[0]?.after).toBe(513);
    expect((await pages(complete))[0]?.documents).toHaveLength(1);
    await complete?.dispose();
  });
  it("rejects a transferred view without its matching resource receipt", async () => {
    const entry = {
      revision: 1,
      cursor: { workspaceId: "a".repeat(32), seq: 1 },
      replyKind: "snapshot",
      viewSnapshot: { processes: { processes: [], activities: [] } },
    };
    await expect(
      new EntityTransferAssembler().receive({
        ...parts([batch(0, 1)])[0],
        payloadKind: "view",
        resource: { kind: "processes", agentId: "one" },
        data: JSON.stringify(entry),
      }),
    ).rejects.toThrow("Resource transfer identity changed");
  });
  it("rejects a terminal generation change between fragments", async () => {
    const assembler = new EntityTransferAssembler();
    const frames = parts([batch(0, 1)], "one", 50).map((frame) => ({
      ...frame,
      payloadKind: "view",
      resource: { kind: "terminal-output", terminalId: "one" },
      cursor: { ...frame.cursor, terminalGeneration: 1 },
    }));
    await assembler.receive(frames[0]);
    await expect(
      assembler.receive({
        ...frames[1],
        cursor: { ...frames[1]!.cursor, terminalGeneration: 2 },
      }),
    ).rejects.toThrow("Entity transfer identity changed");
  });
  it("new epoch discards the interrupted transfer without publishing old pages", async () => {
    const storage = new MemoryStaging();
    const assembler = new EntityTransferAssembler(storage);
    const old = parts([batch(0, 1)], "one", 50);
    await assembler.receive(old[0]);
    const next = parts([batch(0, 1)]).map((frame) => ({
      ...frame,
      epoch: "new",
      transferId: "new",
    }));
    const complete = await assembler.receive(next[0]);
    expect(complete?.epoch).toBe("new");
    expect([...storage.rows.keys()].some((key) => key.startsWith("one:"))).toBe(
      false,
    );
  });
});
