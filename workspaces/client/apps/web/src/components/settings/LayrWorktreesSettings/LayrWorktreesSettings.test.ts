import { MantineProvider } from "@mantine/core";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, expect, it, vi } from "vitest";

const fixture = vi.hoisted(() => ({ configuration: null as unknown }));
vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useState: (initial: unknown) => [
    initial === null
      ? fixture.configuration
      : typeof initial === "function"
        ? initial()
        : initial,
    vi.fn(),
  ],
  useEffect: () => {},
}));
const owner = {
  nativeLayrSettings: () => ({
    current: { status: "live", data: fixture.configuration },
  }),
};
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerStore: () => owner,
    useServerApi: () => ({ post: vi.fn() }),
    useServerFileDialogs: () => ({
      canPickDirectory: () => false,
      pickDirectory: vi.fn(),
    }),
  }),
);
import { LayrWorktreesSettings } from "./LayrWorktreesSettings.tsx";
import { studioSettingsTabs } from "../support/StudioSettingsTabs.tsx";

beforeEach(() => {
  fixture.configuration = null;
});
function render(configuration: unknown) {
  fixture.configuration = configuration;
  return renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(LayrWorktreesSettings, { active: true }),
    ),
  );
}
it("lists the settings tab and displays the installed version and store", () => {
  expect(studioSettingsTabs).toContainEqual([
    "layr-worktrees",
    "Layr & worktrees",
  ]);
  const html = render({
    version: { version: "0.14.2", build: "a6879be" },
    storeRoot: "/store",
  });
  expect(html).toContain("0.14.2 (a6879be)");
  expect(html).toContain("Default store directory");
  expect(html).toContain("Choose a folder");
  expect(html).toContain("Reset");
});
it("displays the missing binary", () => {
  expect(render({ version: null, storeRoot: "/store" })).toContain(
    "Not installed",
  );
});
it("displays an unavailable version and the inspection error", () => {
  const html = render({
    version: null,
    storeRoot: "/store",
    error: "Inspection failed",
  });
  expect(html).toContain("Unavailable");
  expect(html).toContain("Inspection failed");
  expect(html).not.toContain("Not installed");
});
it("displays the store error", () => {
  const html = render({
    version: null,
    storeRoot: null,
    error: "Invalid settings",
  });
  expect(html).toContain("Unavailable");
  expect(html).toContain("Invalid settings");
});

it("renders the server readiness reason", () => {
  expect(
    render({
      version: null,
      storeRoot: "/store",
      readiness: {
        status: "notReady",
        reason: "The store service is not running",
      },
    }),
  ).toContain("The store service is not running");
});
