import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import { describe, expect, it, vi } from "vitest";
import {
  readWorkerModels,
  createWorkerCatalog,
  supportsDaybreakMode,
  type WorkerModelInfo,
} from "./WorkerModelPicker.tsx";

it.each([false, true])(
  "account selection requires fresh metadata with automatic=%s",
  async (workers) => {
    const read = vi.fn(async () => ({
      data: [{ model: "gpt-6.1-sol", provider: "codex" as const }],
    }));
    const rows = await readWorkerModels(
      {
        scope: "account-choice",
        read,
      },
      "selected-account",
      workers,
    );
    expect(read).toHaveBeenCalledExactlyOnceWith({
      query: {
        account_key: "selected-account",
        workers: workers ? "1" : undefined,
        fresh: "1",
      },
    });
    expect(rows.map((row) => row.model)).toEqual(["gpt-6.1-sol"]);
  },
);

const modelInfo = (
  ...args: [] | [WorkerModelInfo["availableAccessPrograms"]]
): WorkerModelInfo => {
  const base = {
    model: "claude-sonnet",
    serviceTiers: [],
    supportedReasoningEfforts: [],
  } satisfies WorkerModelInfo;
  return args.length ? { ...base, availableAccessPrograms: args[0] } : base;
};

describe("supportsDaybreakMode", () => {
  it("keeps standard mode available when the cyber catalog field is absent", () => {
    expect(supportsDaybreakMode(modelInfo(), false)).toBe(true);
    expect(supportsDaybreakMode(modelInfo(null), false)).toBe(false);
  });

  it("requires explicit standard access when cyber metadata is present", () => {
    expect(
      supportsDaybreakMode(modelInfo({ cyber: ["standard"] }), false),
    ).toBe(true);
    expect(
      supportsDaybreakMode(modelInfo({ cyber: ["daybreakBlue"] }), false),
    ).toBe(false);
  });

  it("consumes the generated catalog contract", () => {
    const valid = {
      model: "future-model",
      supportedReasoningEfforts: [{ reasoningEffort: "future-effort" }],
      serviceTiers: [],
    } satisfies WorkerModelInfo;
    expect(valid.supportedReasoningEfforts[0]?.reasoningEffort).toBe(
      "future-effort",
    );
    // @ts-expect-error Known catalog booleans must not widen to opaque metadata.
    const invalidHidden: WorkerModelInfo = { ...valid, hidden: "yes" };
    const invalidProvider: WorkerModelInfo = {
      ...valid,
      // @ts-expect-error Catalog provider identity comes from ProviderKind.
      provider: "future-provider",
    };
    const invalidEffort: WorkerModelInfo = {
      ...valid,
      // @ts-expect-error Upstream-owned effort names stay strings, not untyped JSON.
      supportedReasoningEfforts: [{ reasoningEffort: 1 }],
    };
    expect(invalidHidden.hidden).toBe("yes");
    expect(invalidProvider.provider).toBe("future-provider");
    expect(invalidEffort.supportedReasoningEfforts[0]?.reasoningEffort).toBe(1);
  });
});

it("replacement stores never inherit catalog retry errors or pending completions", async () => {
  const old = observableFixture(),
    replacement = observableFixture();
  const pending =
    Promise.withResolvers<
      Awaited<
        ReturnType<import("./WorkerModelPicker.tsx").ModelCatalogSource["read"]>
      >
    >();
  const reader = {
    scope: "same-catalog-owner",
    read: vi
      .fn()
      .mockRejectedValueOnce(new Error("old failure"))
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValue({ data: [] }),
  };
  const oldCatalog = createWorkerCatalog(old.store, reader, "default");
  const current = createWorkerCatalog(replacement.store, reader, "default");
  try {
    await oldCatalog.retry();
    expect(oldCatalog.error).toBe("old failure");
    expect(current.error).toBe("");
    const oldRequest = oldCatalog.retry();
    await current.retry();
    expect(reader.read).toHaveBeenCalledTimes(3);
    pending.reject(new Error("late old failure"));
    await oldRequest;
    expect(oldCatalog.error).toBe("late old failure");
    expect(current.error).toBe("");
  } finally {
    old.store.dispose();
    replacement.store.dispose();
  }
});
