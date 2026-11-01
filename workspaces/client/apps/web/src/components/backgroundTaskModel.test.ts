import {
  BackgroundTaskIndicator,
  BackgroundTaskKind,
  BackgroundTaskKindLabel,
  BackgroundTaskLabel,
  ProcessSourceKind,
} from "../generated/api";
import { TaskKind, TaskStatus } from "../generated/api";
import type { components } from "../generated/api";
import type { DeepReadonly } from "../deepReadonly";
import { expectTypeOf, it, expect } from "vitest";
import {
  activeTask,
  isCommandTask,
  processIdentity,
  type DisplayBackgroundTask,
  type DisplayProcessTask,
} from "./backgroundTaskModel";

const display = {
  kind: BackgroundTaskKind.command,
  kindLabel: BackgroundTaskKindLabel.command,
  title: "Command",
  active: true,
  indicator: BackgroundTaskIndicator.working,
  label: BackgroundTaskLabel.running,
  inBackgroundList: true,
  actions: ["readOutput"],
} satisfies components["schemas"]["BackgroundTaskDisplay"];

it("uses the server's task-list membership and activity", () => {
  const task = {
    id: "task",
    agent: "lead",
    created: 1,
    status: TaskStatus.running,
    kind: TaskKind.tool,
    display,
  } satisfies components["schemas"]["TaskEntityDto"];
  const hidden = {
    ...task,
    id: "hidden",
    display: { ...display, inBackgroundList: false },
  };
  const monitor = {
    id: "monitor",
    agent: "lead",
    created: 1,
    status: TaskStatus.completed,
    display: {
      ...display,
      kind: BackgroundTaskKind.monitor,
      kindLabel: BackgroundTaskKindLabel.monitor,
      active: false,
      indicator: BackgroundTaskIndicator.unread,
      label: BackgroundTaskLabel.completed,
    },
  } satisfies components["schemas"]["MonitorEntityDto"];
  expect([monitor, task].map(activeTask)).toEqual([false, true]);
  expect(hidden.display.inBackgroundList).toBe(false);
  expect(isCommandTask(task)).toBe(true);
});

it("preserves a server-provided process identity and presentation", () => {
  const process = {
    id: "process / arbitrary:identifier",
    key: "process:key / 1",
    generation: "generation / 2",
    agent: "agent / 3",
    accountKey: "account / 4",
    epoch: 0,
    name: "name",
    created: 1,
    status: TaskStatus.running,
    source: ProcessSourceKind.monitor,
    tail: "output",
    bytes: 6,
    cancelRequested: false,
    outputTruncated: false,
    outputPatterns: [],
    display: {
      ...display,
      kind: BackgroundTaskKind.backgroundTask,
      kindLabel: BackgroundTaskKindLabel.tool,
      title: "Server title",
    },
  } satisfies DisplayProcessTask;
  expect(process.display.kind).toBe("backgroundTask");
  expect(processIdentity(process)).toEqual({
    key: process.key,
    generation: process.generation,
    agent: process.agent,
    accountKey: process.accountKey,
    epoch: process.epoch,
  });
  expectTypeOf<DisplayBackgroundTask>().toEqualTypeOf<
    DeepReadonly<components["schemas"]["BackgroundTaskView"]["value"]>
  >();
});
