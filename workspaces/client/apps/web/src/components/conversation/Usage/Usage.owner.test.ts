// @vitest-environment happy-dom
import { action, autorun } from "mobx";
import { expect, it } from "vitest";
import { ownerFixture } from "../../../app/conversation/ownerFixture.ts";

it("owns one usage store per selected conversation and disposes it on switch", async () => {
  const f = action(ownerFixture)();
  const stop = f.selected.start();
  let stopInterest = () => {};
  try {
    const first = f.selected.conversation;
    expect(first?.usageStore).toBeDefined();
    if (!first) throw new Error("Selected conversation missing");
    const usage = first.usageStore;
    const interest = usage.interests.get("default");
    expect(interest).toBeDefined();
    stopInterest = autorun(() => {
      void interest?.error;
    });
    await Promise.resolve();
    expect(f.post).toHaveBeenCalledExactlyOnceWith("/api/costs/interest", {
      account_key: "default",
    });
    usage.setFeedback({
      key: "account",
      confirm: "credit",
      error: "old error",
      notice: "",
    });
    action(() => {
      f.areas.layout.geometry.viewport.width = 1200;
    })();
    expect(f.selected.conversation?.usageStore).toBe(usage);
    expect(usage.interests.get("default")).toBe(interest);
    expect(f.post).toHaveBeenCalledTimes(1);
    action(() => {
      f.navigation.selectedId = "next";
    })();
    expect(usage.active).toBe(false);
    expect(usage.interests.size).toBe(0);
    const next = f.selected.conversation?.usageStore;
    expect(next).toBeDefined();
    expect(next).not.toBe(usage);
    expect(next?.feedback.error).toBe("");
    expect(next?.resetPending).toBe(false);
  } finally {
    stopInterest();
    await Promise.resolve();
    stop();
    f.dispose();
  }
});
