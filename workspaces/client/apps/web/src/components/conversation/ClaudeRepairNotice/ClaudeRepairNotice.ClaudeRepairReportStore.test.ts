import { action, autorun, observable } from "mobx";
import * as api from "../../../servers/net/api.ts";
import { afterEach, expect, it, vi } from "vitest";
import { ClaudeRepairReportStore } from "./ClaudeRepairReportStore.ts";

it("loads upon observation, hides old scope evidence without a read, and explicitly refreshes the current scope", async () => {
  const agent = observable({
    id: "agent",
    threadId: "thread",
    epoch: 1,
    accountKey: "one",
  });
  const get = vi.spyOn(api, "get").mockResolvedValue({ status: "none" });
  const store = new ClaudeRepairReportStore(api, () => agent);
  expect(get).not.toHaveBeenCalled();
  const stop = autorun(() => store.current);
  await vi.waitFor(() => expect(store.current?.report?.status).toBe("none"));
  expect(get).toHaveBeenCalledTimes(1);
  action(() => agent.epoch++)();
  expect(store.current).toBeUndefined();
  expect(get).toHaveBeenCalledTimes(1);
  await store.refresh();
  expect(store.current?.report?.status).toBe("none");
  expect(get).toHaveBeenCalledTimes(2);
  stop();
  store.dispose();
});

afterEach(() => vi.restoreAllMocks());

it("keeps the latest explicit refresh when the initial report arrives last", async () => {
  const report = {
    status: "none",
    skippedRows: 0,
    invalidSequence: false,
    scopeConfirmed: true,
    nativeHistoryConfirmed: true,
  } satisfies import("./claudeRepairModel.ts").ClaudeRepairReport;
  const pending = Promise.withResolvers<typeof report>();
  vi.spyOn(api, "get")
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValueOnce({ ...report, status: "repaired" });
  const store = new ClaudeRepairReportStore(api, () => ({
    id: "agent",
    threadId: "thread",
  }));
  const stop = autorun(() => store.current);
  await Promise.resolve();
  await store.refresh();
  pending.resolve(report);
  await pending.promise;
  await Promise.resolve();
  expect(store.current?.report?.status).toBe("repaired");
  stop();
  store.dispose();
});
