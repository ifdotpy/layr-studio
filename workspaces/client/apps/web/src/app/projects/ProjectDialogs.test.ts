import { setStartupFixture } from "../../components/shell/integration-support/observableFixture.tsx";
import { componentSnapshot as snapshotFixture } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { act, createElement, isValidElement, type ComponentProps } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { ScreenOwner, ScreenSelection } from "../startup/ScreenOwner.ts";
import FilePreview from "../../components/conversation/FilePreview/FilePreview.tsx";
import { MantineProvider } from "@mantine/core";
import { ServerAreas } from "../ServerAreasProvider.tsx";
import { createServerServices } from "../../servers/serverServices.ts";
import { observableFixture } from "../../components/shell/integration-support/observableFixture.tsx";
import ProjectDirectoryPicker from "../../components/projects/ProjectDirectoryPicker/ProjectDirectoryPicker.tsx";
import ProjectServerCards from "../../components/projects/ProjectServerCards/ProjectServerCards.tsx";
import ProjectFoldersDialog from "../../components/projects/ProjectFoldersDialog/ProjectFoldersDialog.tsx";
import {
  fixtureGetResult,
  fixturePostResult,
} from "../../../tests/misc-fixture.ts";
import type { GetResult, PostResult } from "../../api.ts";

const h = vi.hoisted(() => ({ area: undefined as ServerAreas | undefined }));
vi.mock(
  "../../components/shell/ServerAreasProvider/ServerAreasProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../components/shell/ServerAreasProvider/ServerAreasProvider.tsx")
    >()),
    useServerAreas: () => h.area,
  }),
);
const cleanup: (() => void)[] = [];
afterEach(() => {
  cleanup
    .splice(0)
    .reverse()
    .forEach((stop) => stop());
  vi.restoreAllMocks();
});
const DialogTitle = observer(function DialogTitle() {
  return createElement("div", null, h.area?.ui.descriptor?.title);
});
async function fixture(
  screen: ServerAreas["scope"]["screen"] = { active: true, channel: null },
) {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const services = createServerServices({
    id: `dialogs-${crypto.randomUUID()}`,
    origin: "https://fixture.invalid",
    credentialId: "fixture",
    label: "Fixture",
  });
  const models = observableFixture([{ id: "agent", hasBackgroundWork: false }]);
  const area = new ServerAreas({
    services,
    store: models.store,
    screen,
  });
  setStartupFixture(
    area,
    Object.assign(snapshotFixture(), {
      token: "",
      stateDir: "/fixture",
    }),
  );
  h.area = area;
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  cleanup.push(() => {
    act(() => root.unmount());
    area.dispose();
    services.close();
    models.store.dispose();
    host.remove();
  });
  await act(async () => root.render(createElement(DialogTitle)));
  return { area, host, models, root };
}
const Preview = observer(function Preview({ area }: { area: ServerAreas }) {
  return area.scope.screen.active
    ? createElement(FilePreview, {
        target: area.ui.filePreview,
        onClose: area.ui.closePreview,
      })
    : null;
});
it.each(["pending", "settled"])(
  "leaving server A closes its %s preview before returning from B",
  async (state) => {
    const selected = new ScreenSelection(["A", "B"]),
      screen = new ScreenOwner();
    screen.connect(null, () => selected.current === "A");
    const { area, root } = await fixture(screen);
    cleanup.push(area.ui.start());
    const pending = Promise.withResolvers<GetResult<"/api/file">>();
    const get = vi
      .spyOn(area.scope.services.api, "get")
      .mockReturnValue(pending.promise);
    vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:server");
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    await act(async () => {
      root.render(
        createElement(MantineProvider, null, createElement(Preview, { area })),
      );
      area.ui.showPreview({ asset: "file" });
    });
    if (state === "settled")
      await act(async () => {
        pending.resolve({
          name: "old-file",
          mime: "text/plain",
          base64: "YQ==",
        });
        await pending.promise;
      });
    await act(async () => selected.select("B"));
    expect(area.ui.filePreview).toBeNull();
    if (state === "pending")
      await act(async () => {
        pending.resolve({
          name: "old-file",
          mime: "text/plain",
          base64: "YQ==",
        });
        await pending.promise;
      });
    await act(async () => selected.select("A"));
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    expect(get).toHaveBeenCalledTimes(1);
  },
);
it("opens the deletion confirmation through the committed React dialog state", async () => {
  const { area, host } = await fixture();
  await act(async () => area.ui.requestDelete({ id: "agent" }));
  expect(host.textContent).toBe("Delete chat?");
});
it("a completed folder write cannot close a newer dialog", async () => {
  const { area, host, models } = await fixture();
  const response = Promise.withResolvers<PostResult<"/api/conversation">>();
  vi.spyOn(area.scope.services.api, "post").mockReturnValueOnce(
    response.promise,
  );
  await act(async () =>
    area.ui.changeProjectFolder(models.mirror.current.agents.get("agent")!),
  );
  const body = area.ui.descriptor?.body;
  if (!isValidElement<ComponentProps<typeof ProjectDirectoryPicker>>(body))
    throw new Error("Missing folder picker");
  const writing = body.props.onSelect("/new-folder", "local");
  await act(async () => area.ui.openAddProject());
  response.resolve(fixturePostResult<"/api/conversation">({}));
  await act(async () => writing);
  expect(host.textContent).toBe("Add project");
});
it("discovers servers before mounting cards and ignores superseded discovery", async () => {
  const { area, host } = await fixture();
  const response = Promise.withResolvers<GetResult<"/api/multi-server">>();
  const get = vi
    .spyOn(area.scope.services.api, "get")
    .mockReturnValueOnce(response.promise);
  let choosing: Promise<void> | undefined;
  await act(async () => {
    choosing = area.ui.chooseProject("/project");
  });
  expect(host.textContent).toBe("");
  const reply = fixtureGetResult<"/api/multi-server">({
    identity: {
      serverId: "home",
      label: "Home",
      origin: null,
      publicKey: "fixture",
      tailscaleUser: null,
    },
    servers: [],
  });
  await act(async () => {
    response.resolve(reply);
    await choosing;
  });
  const cards = area.ui.descriptor?.body;
  if (!isValidElement<ComponentProps<typeof ProjectServerCards>>(cards))
    throw new Error("Missing server cards");
  expect(cards.type).toBe(ProjectServerCards);
  expect(cards.props.servers[0]?.id).toBe("local");
  const late = Promise.withResolvers<typeof reply>();
  get.mockReturnValueOnce(late.promise);
  await act(async () => {
    choosing = area.ui.chooseProject("/project");
  });
  await act(async () => area.ui.openAddProject());
  await act(async () => {
    late.resolve(reply);
    await choosing;
  });
  expect(host.textContent).toBe("Add project");
});

it("passes the canonical streamed project model to the project settings dialog", async () => {
  const { area, models } = await fixture();
  const project = { id: "project", path: "/project", name: "Project" };
  models.mirror.apply([
    { collection: "project", id: project.id, value: project },
  ]);
  vi.spyOn(area.scope.services.api, "get").mockResolvedValueOnce(
    fixtureGetResult<"/api/projects">({ items: [project] }),
  );
  await act(async () => area.ui.showFolders(project.id));
  const body = area.ui.descriptor?.body;
  if (!isValidElement<ComponentProps<typeof ProjectFoldersDialog>>(body))
    throw new Error("Missing project settings");
  expect(body.type).toBe(ProjectFoldersDialog);
  expect(body.props.project).toBe(
    models.mirror.current.projects.get(project.id),
  );
});

it("a first send opens project choice through the real creation owner and retains its draft", async () => {
  const { area, host } = await fixture();
  vi.spyOn(area.scope.services.api, "get").mockResolvedValueOnce(
    fixtureGetResult<"/api/multi-server">({
      identity: {
        serverId: "home",
        label: "Home",
        origin: null,
        publicKey: "fixture",
        tailscaleUser: null,
      },
      servers: [],
    }),
  );
  const durable = vi.spyOn(area.scope.services.outbox, "durableSend");
  area.delivery.editDraft("First message", "new");
  await act(async () => area.delivery.send());
  expect(host.textContent).toBe("New chat");
  expect(area.scope.services.drafts.getDraft("new")).toBe("First message");
  expect(durable).not.toHaveBeenCalled();
});
