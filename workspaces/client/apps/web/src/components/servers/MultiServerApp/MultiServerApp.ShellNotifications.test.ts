// @vitest-environment happy-dom
import { action, observable } from "mobx";
import { expect, it, vi } from "vitest";
import type { DesktopAlert } from "../../../desktop/desktopAlerts.ts";
import * as alerts from "../../../desktop/desktopAlerts.ts";
import { createProjectionMetadataMirror } from "../../../generated/observableResources.ts";
import { startDesktopNotifications } from "../../../hooks/desktopNotifications.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";

const notify = vi.hoisted(() => vi.fn(async () => true));
vi.mock("../../../native/notifications.ts", async (original) => ({
  ...(await original<typeof import("../../../native/notifications.ts")>()),
  hasDesktopNotifications: () => true,
  notifyDesktop: notify,
}));

it("keeps notification baselines and disposal independent for identical alert ids on two servers", () => {
  const first = observableFixture();
  const second = observableFixture();
  const firstAlerts = observable.box<DesktopAlert[]>([], { deep: false });
  const secondAlerts = observable.box<DesktopAlert[]>([], { deep: false });
  const read = vi
    .spyOn(alerts, "desktopAlerts")
    .mockImplementation((store) =>
      store === first.store ? firstAlerts.get() : secondAlerts.get(),
    );
  const metadata = createProjectionMetadataMirror({
    complete: true,
    projectSequences: {},
    ready: true,
    workspaceId: "fixture",
  });
  const start = (store: typeof first.store, id: string) =>
    startDesktopNotifications({
      store,
      metadata: metadata.current,
      enabled: true,
      opened: null,
      server: { id, label: id },
    });
  const stopFirst = start(first.store, "first");
  const stopSecond = start(second.store, "second");
  const event: DesktopAlert = {
    id: "same",
    title: "Answer",
    body: "Ready",
    target: { agentId: "chat", section: "messages" },
  };
  try {
    expect(notify).not.toHaveBeenCalled();
    action(() => {
      firstAlerts.set([event]);
      secondAlerts.set([event]);
    })();
    expect(notify.mock.calls).toEqual([
      [
        expect.objectContaining({
          title: "first: Answer",
          target: { ...event.target, serverId: "first" },
        }),
      ],
      [
        expect.objectContaining({
          title: "second: Answer",
          target: { ...event.target, serverId: "second" },
        }),
      ],
    ]);
    action(() => firstAlerts.set([{ ...event }]))();
    expect(notify).toHaveBeenCalledTimes(2);
    stopFirst();
    action(() => {
      firstAlerts.set([{ ...event, id: "next" }]);
      secondAlerts.set([event, { ...event, id: "next" }]);
    })();
    expect(notify).toHaveBeenCalledTimes(3);
    expect(notify).toHaveBeenLastCalledWith(
      expect.objectContaining({
        target: { ...event.target, serverId: "second" },
      }),
    );
  } finally {
    stopFirst();
    stopSecond();
    first.store.dispose();
    second.store.dispose();
    read.mockRestore();
    notify.mockClear();
  }
});

it("does not replay a direct-mode alert when the combined notifier resumes", () => {
  const fixture = observableFixture();
  const direct = observable.box(false);
  const current = observable.box<DesktopAlert[]>([], { deep: false });
  const read = vi
    .spyOn(alerts, "desktopAlerts")
    .mockImplementation(() => current.get());
  const metadata = createProjectionMetadataMirror({
    complete: true,
    projectSequences: {},
    ready: true,
    workspaceId: "fixture",
  });
  const start = (combined: boolean) =>
    startDesktopNotifications({
      store: fixture.store,
      metadata: metadata.current,
      opened: null,
      get enabled() {
        return combined !== direct.get();
      },
      server: { id: "local", label: "Local" },
    });
  const stopCombined = start(true),
    stopDirect = start(false);
  try {
    action(() => direct.set(true))();
    action(() =>
      current.set([
        {
          id: "handoff",
          title: "Answer",
          body: "Ready",
          target: { agentId: "chat", section: "messages" },
        },
      ]),
    )();
    expect(notify).toHaveBeenCalledTimes(1);
    action(() => direct.set(false))();
    expect(notify).toHaveBeenCalledTimes(1);
  } finally {
    stopCombined();
    stopDirect();
    fixture.store.dispose();
    read.mockRestore();
    notify.mockClear();
  }
});
