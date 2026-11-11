// @vitest-environment happy-dom
import { autorun } from "mobx";
import { expect, it, vi } from "vitest";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import * as api from "../../../servers/net/api.ts";
import { createCostInterest, createUsageView } from "./Usage.tsx";
import type { CostsData } from "../../../generated/observableResources.ts";

it("shares one cost-interest request across observers and suppresses a disposed result", async () => {
  let rejectRequest: (error: Error) => void = () => {};
  const post = vi.fn(
    () =>
      new Promise<never>((_resolve, reject) => {
        rejectRequest = reject;
      }),
  );
  const costs = {
    current: { status: "live" as const, data: [] satisfies CostsData },
  };
  const interest = createCostInterest(
    { post },
    { costs: () => costs },
    "account",
  );
  expect(post).not.toHaveBeenCalled();
  const errors: string[] = [];
  const first = autorun(() => {
    errors.push(interest.error);
  });
  const second = autorun(() => {
    void interest.error;
  });
  await Promise.resolve();
  expect(post).toHaveBeenCalledExactlyOnceWith("/api/costs/interest", {
    account_key: "account",
  });
  first();
  expect(post).toHaveBeenCalledOnce();
  second();
  rejectRequest(new Error("late failure"));
  await Promise.resolve();
  await Promise.resolve();
  expect(interest.error).toBe("");
  expect(errors).toEqual([""]);
});

it("the reset owner is active only while its selected lifetime is started", () => {
  const models = observableFixture();
  const state = createUsageView(api, models.store);
  state.start();
  const stop = state.dispose;
  expect(state.active).toBe(true);
  stop();
  expect(state.active).toBe(false);
  state.start();
  const resume = state.dispose;
  expect(state.active).toBe(true);
  resume();
  state.dispose();
  models.store.dispose();
});

it("ignores a cost-interest failure from before an observation gap", async () => {
  const failures: Array<(error: Error) => void> = [];
  const post = vi.fn(
    () => new Promise<never>((_resolve, reject) => failures.push(reject)),
  );
  const interest = createCostInterest(
    { post },
    { costs: () => ({ current: { status: "live", data: [] } }) },
    "account",
  );
  const stop = autorun(() => {
    void interest.error;
  });
  await Promise.resolve();
  stop();
  await Promise.resolve();
  const resume = autorun(() => {
    void interest.error;
  });
  try {
    await Promise.resolve();
    expect(post).toHaveBeenCalledTimes(2);
    failures[0]!(new Error("obsolete failure"));
    await Promise.resolve();
    await Promise.resolve();
    expect(interest.error).toBe("");
    failures[1]!(new Error("current failure"));
    await Promise.resolve();
    await Promise.resolve();
    expect(interest.error).toBe("current failure");
  } finally {
    resume();
  }
});
