// @vitest-environment happy-dom
import { action, autorun } from "mobx";
import { expect, it, vi } from "vitest";
import { ownerFixture } from "./ownerFixture.ts";

it("owns display-phase smoothing before a transcript renders and cancels it at disposal", () => {
  vi.useFakeTimers();
  const f = ownerFixture();
  const stop = f.selected.start();
  const detail = f.models.details.get("lead")!;
  try {
    detail.apply({ ...detail.current, activity: { phase: "tool" } });
    expect(f.selected.displayPhase.current).toBe("thinking");
    vi.advanceTimersByTime(250);
    expect(f.selected.displayPhase.current).toBe("tool");
    detail.apply({ ...detail.current, activity: { phase: "reasoning" } });
    stop();
    vi.advanceTimersByTime(250);
    expect(f.selected.displayPhase.current).toBe("tool");
  } finally {
    f.dispose();
    vi.useRealTimers();
  }
});

it("creates prompt and voice owners once per selected conversation and disposes the departed voice", () => {
  const f = ownerFixture();
  const stop = f.selected.start();
  const first = f.selected.conversation!;
  const voiceDisposed = vi.spyOn(first.voice, "dispose");
  const seen: string[][] = [];
  const observe = autorun(() =>
    seen.push(first.promptNavigation.prompts.map((prompt) => prompt.text)),
  );
  try {
    action(() => {
      f.selected.history.items = [
        { id: "prompt", role: "user", text: "First prompt" },
      ];
    })();
    expect(seen.at(-1)).toEqual(["First prompt"]);
    expect(f.selected.conversation?.promptNavigation).toBe(
      first.promptNavigation,
    );
    f.navigation.selectExternal("other", { requestId: "switch" });
    expect(f.selected.conversation).not.toBe(first);
    expect(voiceDisposed).toHaveBeenCalledOnce();
  } finally {
    observe();
    stop();
    voiceDisposed.mockRestore();
    f.dispose();
  }
});
