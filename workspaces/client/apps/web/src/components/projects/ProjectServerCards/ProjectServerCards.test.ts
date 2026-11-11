// @vitest-environment happy-dom
import type { ComponentProps } from "react";
import { afterEach } from "vitest";
import { ProjectServerSelection } from "../../../app/projects/CreationOwner.ts";
import {
  createServerServices,
  type ServerServices,
} from "../../../servers/serverServices.ts";
import { localServer } from "../../../servers/registry.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { describe, expect, it } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { theme } from "../../../theme.ts";
import ProjectServerCards from "./ProjectServerCards.tsx";
import { ChatWorkspaceMode } from "../../../generated/api.ts";

describe("new chat actions", () => {
  it("offers Cancel and a primary Start chat action", () => {
    const markup = renderToStaticMarkup(
      createElement(
        ServerStoreProvider,
        { value: observableFixture().store },
        createElement(
          MantineProvider,
          { theme },
          cards({
            project: {
              id: "logical",
              path: "logical",
              name: "Project",
              locations: [
                { serverId: "local", path: "/folder", projectId: "/folder" },
              ],
            },
            servers: [
              {
                id: "local",
                label: "This computer",
                system: "Darwin",
              },
            ],
            onChoose: () => {},
            onAdd: () => {},
            onCancel: () => {},
          }),
        ),
      ),
    );
    expect(markup).toContain("Workspace");
    expect(markup).toContain("Layr");
    expect(
      markup.match(/<input(?=[^>]*value="layr-native")[^>]*>/)?.[0],
    ).toContain('disabled=""');
    expect(markup).toContain("WT");
    expect(markup).toContain('aria-label="Agent role" role="radiogroup"');
    expect(markup).toMatch(/<input[^>]*type="radio"[^>]*value="worker"/);
    expect(markup).toMatch(/<input[^>]*type="radio"[^>]*value="review"/);
    expect(markup).toContain('role="listbox" aria-label="Main agent model"');
    expect(markup).toContain("Cancel");
    expect(markup).toContain("Start chat");
    expect(markup).toMatch(/data-variant="filled"[^>]*>[\s\S]*?Start chat/);
  });
});

it("allows a store override when the default store is unavailable", () => {
  const markup = renderToStaticMarkup(
    createElement(
      ServerStoreProvider,
      { value: observableFixture().store },
      createElement(
        MantineProvider,
        { theme },
        cards({
          project: {
            id: "linux",
            name: "Linux",
            path: "/folder",
            locations: [
              { serverId: "linux", projectId: "linux", path: "/folder" },
            ],
          },
          servers: [
            {
              id: "linux",
              label: "Linux",
              system: "Linux",
              nativeLayr: {
                status: "notReady",
                reason: "A btrfs store is required.",
              },
              nativeLayrMessage: "A btrfs store is required.",
              defaultWorkspaceMode: ChatWorkspaceMode.worktree,
            },
          ],
          onChoose: () => {},
          onAdd: () => {},
          onCancel: () => {},
        }),
      ),
    ),
  );
  expect(markup).toContain("A btrfs store is required.");
  expect(markup).toContain(
    'class="native-layr-error" title="A btrfs store is required."',
  );
  expect(
    markup.match(/<input(?=[^>]*value="layr-native")[^>]*>/)?.[0],
  ).not.toContain('disabled=""');
  expect(markup.match(/<input(?=[^>]*value="worktree")[^>]*>/)?.[0]).toContain(
    'checked=""',
  );
});

it("shows the server's install message on the disabled Layr option", () => {
  const markup = renderToStaticMarkup(
    createElement(
      ServerStoreProvider,
      { value: observableFixture().store },
      createElement(
        MantineProvider,
        { theme },
        cards({
          project: { id: "p", name: "P", path: "/p" },
          servers: [
            {
              id: "local",
              label: "Server",
              system: "Darwin",
              nativeLayr: { status: "notInstalled" },
              nativeLayrMessage: "Install layr",
              defaultWorkspaceMode: ChatWorkspaceMode.worktree,
            },
          ],
          onChoose: () => {},
          onAdd: () => {},
          onCancel: () => {},
        }),
      ),
    ),
  );
  expect(markup).toContain('class="native-layr-error" title="Install layr"');
  expect(
    markup.match(/<input(?=[^>]*value="layr-native")[^>]*>/)?.[0],
  ).toContain('disabled=""');
});

it("selects the server's default when Layr is ready", () => {
  const markup = renderToStaticMarkup(
    createElement(
      ServerStoreProvider,
      { value: observableFixture().store },
      createElement(
        MantineProvider,
        { theme },
        cards({
          project: { id: "p", name: "P", path: "/p" },
          servers: [
            {
              id: "local",
              label: "Server",
              system: "Linux",
              nativeLayr: { status: "ready" },
              defaultWorkspaceMode: ChatWorkspaceMode["layr-native"],
            },
          ],
          onChoose: () => {},
          onAdd: () => {},
          onCancel: () => {},
        }),
      ),
    ),
  );
  const option = markup.match(/<input(?=[^>]*value="layr-native")[^>]*>/)?.[0];
  expect(option).toContain('checked=""');
  expect(option).not.toContain('disabled=""');
  expect(markup).not.toContain("Store directory (optional)");
});

const services: ServerServices[] = [];
afterEach(() => services.splice(0).forEach((service) => service.close()));
function cards(
  props: Omit<ComponentProps<typeof ProjectServerCards>, "owner">,
) {
  const service = createServerServices(localServer());
  services.push(service);
  const owner = new ProjectServerSelection(
    props.project,
    () => props.servers,
    undefined,
    () => service,
  );
  return createElement(ProjectServerCards, { ...props, owner });
}
