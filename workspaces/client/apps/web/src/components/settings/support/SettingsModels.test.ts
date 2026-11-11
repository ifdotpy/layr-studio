import { autorun } from "mobx";
import { expect, it, vi } from "vitest";
import { OrchestrationSettingsStore } from "../../../app/settings/SettingsModels.ts";

it("ignores a response after its last settings observer leaves", async () => {
  let finish!: (value: {
    settings: {
      childResultCoalesceSeconds: number;
      idleSessionUnloadMinutes: number;
    };
  }) => void;
  const get = vi.fn(
    () =>
      new Promise<typeof model.configuration>((resolve) => {
        finish = resolve;
      }),
  );
  const model = new OrchestrationSettingsStore({ get, post: vi.fn() });
  const stop = autorun(() => model.configuration);
  expect(get).not.toHaveBeenCalled();
  await Promise.resolve();
  expect(get).toHaveBeenCalledTimes(1);
  stop();
  finish({
    settings: { childResultCoalesceSeconds: 7, idleSessionUnloadMinutes: 8 },
  });
  await Promise.resolve();
  expect(model.configuration).toBeNull();
  model.dispose();
});
