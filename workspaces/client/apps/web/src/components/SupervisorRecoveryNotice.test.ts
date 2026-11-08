import { required } from "./componentTestFixtures";
import { afterEach, expect, it, vi } from "vitest";
import type { DesktopResponse } from "../generated/observableResources.ts";
import type { ResourceStatus } from "../store/runtime.ts";
import type { DesktopNotice } from "./desktopNotices";
const mocks = vi.hoisted(() => ({
  desktop: { status: "loading" } as ResourceStatus<DesktopResponse>,
  notices: [] as DesktopNotice[],
  get: vi.fn(),
}));
vi.mock("mobx-react-lite", () => ({
  observer: <T>(component: T) => component,
}));
vi.mock("../servers/ServerServicesContext", () => ({
  useServerStore: () => ({ desktop: () => ({ current: mocks.desktop }) }),
}));
vi.mock("./desktopNotices", async (original) => {
  const source = await original<typeof import("./desktopNotices")>();
  return {
    ...source,
    desktopNotices(data: Parameters<typeof source.desktopNotices>[0]) {
      return (mocks.notices = source.desktopNotices(data));
    },
  };
});
import SupervisorRecoveryNotice from "./SupervisorRecoveryNotice";
afterEach(() => {
  mocks.desktop = { status: "loading" };
  mocks.notices = [];
});
it("renders desktop stream notices and retains the full-disk notice on projection failure", () => {
  for (const level of ["warning", "critical", "full"] as const) {
    mocks.desktop = {
      status: "live",
      data: {
        application: "test",
        backendBuild: "test",
        browser: null,
        liveUpdate: null,
        mobileProtocol: 1,
        nativeRuntime: null,
        pid: 1,
        protocol: 1,
        providerVersions: { checkedAt: null, providers: [], warnings: [] },
        publicOrigin: null,
        restartEnvironment: {},
        stateDir: "/fixture",
        supervisor: null,
        supervisorError: null,
        supervisorFallback: false,
        supervisorMode: false,
        supervisorRequired: false,
        diskSpace: { level, message: `Disk ${level}.` },
        supervisorNotice: "Supervisor stopped.",
      },
    };
    SupervisorRecoveryNotice({});
    expect(mocks.notices).toEqual([
      { id: "disk", level, text: `Disk ${level}.` },
      { id: "supervisor", level: "warning", text: "Supervisor stopped." },
    ]);
  }
  if (mocks.desktop.status !== "live")
    throw new Error("Expected the streamed desktop");
  mocks.desktop = { status: "stale", data: mocks.desktop.data };
  SupervisorRecoveryNotice({});
  expect(required(mocks.notices[0])).toEqual({
    id: "disk",
    level: "full",
    text: "Disk full.",
  });
  expect(mocks.get).not.toHaveBeenCalled();
});
