import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import { required } from "../teams/helpers.ts";
import assert from "node:assert/strict";
import type { TestRuntimeView } from "../playwright.ts";
import type { TestAgent } from "../playwright.ts";
import type { ProjectEntityDto } from "../../src/generated/api.ts";

import type { MultiServerFixtureOptions } from "../servers/multi-server-fixture.ts";
import type { Owner } from "./sidebar-parity-fixture.ts";
import { fixturePostBody } from "../servers/browser-fixture.ts";
import { remoteNetwork } from "../servers/browser-fixture.ts";
import { test, expect } from "../playwright.ts";
import { fixture } from "../servers/multi-server-fixture.ts";
import { sidebarParityFixture } from "./sidebar-parity-fixture.ts";
for (const combined of [false, true]) {
  test(`the sidebar drawer preserves filter, selection, focus, and desktop preference: ${combined ? "combined" : "direct"}`, async ({
    page,
    context,
  }, testInfo) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    const f = await sidebarParityFixture(context, { paired: combined });
    try {
      const sidebar = await f.open(page, { combined });
      const rail = sidebar.getByRole("separator", {
        name: "Conversation sidebar width",
      });
      await rail.press("End");
      await page.evaluate(() => document.fonts.ready);
      const fitBoundary =
        Number(await rail.getAttribute("aria-valuemin")) +
        (await page.evaluate(() => {
          const style = getComputedStyle(document.documentElement);
          return (
            parseFloat(style.getPropertyValue("--layout-conversation-min")) *
            parseFloat(style.fontSize)
          );
        }));
      const preferred = await sidebar.evaluate(
        (node) => node.getBoundingClientRect().width,
      );
      const toggle = page.getByRole("button", {
        name: combined
          ? "Servers and chats"
          : /^(Back to chats|Toggle conversations)$/,
      });
      for (const width of [320, 360, 390, 430]) {
        await page.setViewportSize({ width, height: 844 });
        await expect(toggle).toHaveAccessibleName(
          combined ? "Servers and chats" : "Back to chats",
        );
        await toggle.click();
        await expect(sidebar).toHaveAttribute("data-presentation", "drawer");
        if (width === 320)
          await page.screenshot({
            path: testInfo.outputPath("sidebar-320.png"),
          });
        await sidebar.getByPlaceholder("Filter sidebar").fill("Root A");
        await sidebar
          .getByRole("button", { name: "Close conversations", exact: true })
          .click();
        await expect(sidebar).toBeHidden();
        await expect(toggle).toBeFocused();
        await toggle.click();
        await expect(sidebar.getByPlaceholder("Filter sidebar")).toHaveValue(
          "Root A",
        );
        await page.evaluate(() => {
          window.__sidebarReturnedFocus = false;
          const listener = (event: FocusEvent) => {
            const trigger = document.querySelector(
              "#sidebar-toggle, .server-mobile-toggle",
            );
            if (
              event.target instanceof Node &&
              trigger?.contains(event.target) &&
              (document.querySelector("#sidebar")?.getBoundingClientRect()
                .width ?? 0) > 0
            )
              window.__sidebarReturnedFocus = true;
          };
          document.addEventListener("focusin", listener, { once: false });
        });
        await sidebar
          .getByRole("button", { name: "Local Root A" })
          .first()
          .click();
        await expect(sidebar).toBeHidden();
        // The combined caller then transfers focus to the selected server's composer.
        expect(await page.evaluate(() => window.__sidebarReturnedFocus)).toBe(
          true,
        );
        expect(
          await page.evaluate(
            () =>
              document
                .querySelector("#sidebar")
                ?.contains(document.activeElement) ?? false,
          ),
        ).toBe(false);
      }
      await page.setViewportSize({
        width: Math.ceil(fitBoundary),
        height: 900,
      });
      await expect(rail).toBeVisible();
      await page.setViewportSize({
        width: Math.ceil(fitBoundary) - 1,
        height: 900,
      });
      await toggle.click();
      await expect(sidebar).toHaveAttribute("data-presentation", "drawer");
      await sidebar
        .getByRole("button", { name: "Close conversations", exact: true })
        .click();
      await expect(toggle).toBeFocused();
      await page.setViewportSize({ width: 1440, height: 900 });
      await expect(rail).toBeVisible();
      await expect
        .poll(() =>
          sidebar.evaluate((node) => node.getBoundingClientRect().width),
        )
        .toBe(preferred);
      if (!combined) {
        await sidebar
          .getByRole("button", { name: "Close conversations", exact: true })
          .click();
        await page.setViewportSize({
          width: Math.ceil(fitBoundary) - 1,
          height: 900,
        });
        await page.reload();
        await expect(sidebar).toBeHidden();
        await toggle.click();
        await expect(sidebar).toBeVisible();
        await expect(sidebar).toHaveAttribute("data-presentation", "drawer");
      }
    } finally {
      await f.close();
    }
  });
}

test("the direct sidebar bounds pointer resize, cancels lost capture, and measures large text", async ({
  page,
  context,
}, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const f = await sidebarParityFixture(context, { paired: false });
  try {
    const sidebar = await f.open(page, { combined: false });
    const rail = sidebar.getByRole("separator", {
      name: "Conversation sidebar width",
    });
    const drag = async (x: number) => {
      const box = (await rail.boundingBox())!;
      await page.mouse.move(box.x + box.width / 2, box.y + 80);
      await page.mouse.down();
      await page.mouse.move(x, box.y + 80);
      await page.mouse.up();
    };
    await drag(0);
    await expect
      .poll(() =>
        sidebar.evaluate((node) =>
          Math.round(node.getBoundingClientRect().width),
        ),
      )
      .toBe(Number(await rail.getAttribute("aria-valuemin")));
    await drag(1440);
    await expect
      .poll(() =>
        sidebar.evaluate((node) =>
          Math.round(node.getBoundingClientRect().width),
        ),
      )
      .toBe(Number(await rail.getAttribute("aria-valuemax")));
    const before = await sidebar.evaluate(
      (node) => node.getBoundingClientRect().width,
    );
    const box = (await rail.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + 80);
    await page.mouse.down();
    await page.mouse.move(500, box.y + 80);
    await rail.evaluate((node) =>
      node.dispatchEvent(
        new PointerEvent("lostpointercapture", { bubbles: true, pointerId: 1 }),
      ),
    );
    await page.mouse.up();
    await expect
      .poll(() =>
        sidebar.evaluate((node) => node.getBoundingClientRect().width),
      )
      .toBe(before);
    await page.addStyleTag({
      content:
        ":root { font-size: 20px !important } #sidebar .brand { font-size: 40px !important }",
    });
    await rail.press("Home");
    await expect.poll(() => rail.getAttribute("aria-valuemin")).not.toBe("208");
    expect(
      await sidebar.locator(".sidebar-header").evaluate((header) => {
        const boxes = [...header.children].map((child) =>
          child.getBoundingClientRect(),
        );
        return boxes.every(
          (box) =>
            box.left >= header.getBoundingClientRect().left &&
            box.right <= header.getBoundingClientRect().right,
        );
      }),
    ).toBe(true);
    await page.screenshot({
      path: testInfo.outputPath("sidebar-large-text.png"),
    });
    await expect
      .poll(async () => {
        const side = (await sidebar.boundingBox())!;
        const bar = (await page
          .locator(".terminal-dock:not(.is-open)")
          .boundingBox())!;
        return Math.round(side.width) - Math.round(bar.width);
      })
      .toBe(0);
    const keys = await page.evaluate(() =>
      Object.keys(localStorage).filter((key) =>
        key.startsWith("studio-sidebar-width:"),
      ),
    );
    expect(keys).toHaveLength(0);
    await rail.press("End");
    await expect
      .poll(() =>
        sidebar.evaluate((node) => node.getBoundingClientRect().width),
      )
      .toBe(Number(await rail.getAttribute("aria-valuemax")));
    await page.reload();
    await expect(rail).toBeVisible();
    const defaultWidth = await page.evaluate(() => {
      // Resolve the owner's length through CSS, including browser subpixel quantization.
      const probe = document.createElement("div");
      probe.style.cssText =
        "position:fixed;width:var(--layout-sidebar-preferred);height:0;visibility:hidden;pointer-events:none";
      document.body.append(probe);
      const width = probe.getBoundingClientRect().width;
      probe.remove();
      return width;
    });
    await expect
      .poll(() =>
        sidebar.evaluate((node) => node.getBoundingClientRect().width),
      )
      .toBe(defaultWidth);
  } finally {
    await f.close();
  }
});

test("the combined sidebar resizes, cancels, resets, and reports its effective width", async ({
  page,
  context,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const f = await sidebarParityFixture(context);
  try {
    const sidebar = await f.open(page);
    const rail = sidebar.getByRole("separator", {
      name: "Conversation sidebar width",
    });
    await expect(rail).toBeVisible();
    const initial = await sidebar.evaluate(
      (node) => node.getBoundingClientRect().width,
    );
    await rail.focus();
    await rail.press("ArrowRight");
    await expect
      .poll(() =>
        sidebar.evaluate((node) => node.getBoundingClientRect().width),
      )
      .toBeGreaterThan(initial);
    const preferred = await sidebar.evaluate(
      (node) => node.getBoundingClientRect().width,
    );
    await page.setViewportSize({ width: 880, height: 900 });
    await expect
      .poll(() =>
        sidebar.evaluate((node) => node.getBoundingClientRect().width),
      )
      .toBeLessThan(preferred);
    const bar = page.locator(".shell-terminal-bar");
    await expect
      .poll(async () => Math.round((await bar.boundingBox())!.width))
      .toBe(Math.round((await sidebar.boundingBox())!.width));
    await page.setViewportSize({ width: 1440, height: 900 });
    await expect
      .poll(() =>
        sidebar.evaluate((node) => node.getBoundingClientRect().width),
      )
      .toBe(preferred);
    const box = (await rail.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + 80);
    await page.mouse.down();
    await page.mouse.move(box.x + 150, box.y + 80);
    await page.keyboard.press("Escape");
    await page.mouse.up();
    await expect
      .poll(() =>
        sidebar.evaluate((node) => node.getBoundingClientRect().width),
      )
      .toBe(preferred);
    await rail.press("Home");
    await expect
      .poll(() =>
        sidebar.evaluate((node) => node.getBoundingClientRect().width),
      )
      .toBe(initial);
  } finally {
    await f.close();
  }
});

for (const [theme, viewport] of [
  ["light", { width: 1440, height: 900 }],
  ["dark", { width: 1440, height: 900 }],
  ["mobile", { width: 390, height: 844 }],
] as const) {
  test(`the combined classic sidebar routes actions to their owner: ${theme}`, async ({
    page,
    context,
  }, testInfo) => {
    test.setTimeout(90000);
    page.setDefaultTimeout(10000);
    await page.setViewportSize(viewport);
    await page.emulateMedia({
      colorScheme: theme === "dark" ? "dark" : "light",
    });
    const calls: Record<
      Owner,
      {
        path: string;
        body: import("../servers/browser-fixture.ts").FixtureRequestFields<
          "/api/organization" | "/api/rename" | "/api/projects"
        >;
      }[]
    > = { local: [], remote: [] };
    const handler =
      (owner: Owner): NonNullable<MultiServerFixtureOptions["handle"]> =>
      ({ url, body: rawBody, json, snapshot }) => {
        if (url.pathname === "/api/organization") {
          const body = fixturePostBody<"/api/organization">(rawBody);
          calls[owner].push({ path: url.pathname, body });
          const row = snapshot.threads.find((chat) => chat.id === body.id);
          expect(row).toBeTruthy();

          for (const key of ["pinned", "archived"] as const)
            if (key in body) row![key] = body[key];
          if ("project_folder" in body) {
            row!.projectFolder = body.project_folder;
            row!.projectFolderRevision = (row!.projectFolderRevision || 0) + 1;
          }
          json({
            projectFolder: row!.projectFolder || null,
            pinned: !!row!.pinned,
            archived: !!row!.archived,
          });
          return true;
        }
        if (url.pathname === "/api/rename") {
          const body = fixturePostBody<"/api/rename">(rawBody);
          calls[owner].push({ path: url.pathname, body });
          const row = snapshot.threads.find((chat) => chat.id === body.id);
          expect(row).toBeTruthy();
          row!.name = body.name;
          json({ ok: true });
          return true;
        }
        if (url.pathname === "/api/projects") {
          const body = fixturePostBody<"/api/projects">(rawBody);
          calls[owner].push({ path: url.pathname, body });

          const project = snapshot.runtime.projects.find(
            (row) => row.path === body.path,
          );
          expect(project).toBeTruthy();

          expect(body.expected_revision).toBe(project!.organizationRevision);
          if (body.action === "rename_folder")
            project!.folders!.find((row) => row.id === body.folder_id)!.name =
              body.name!;
          project!.organizationRevision! += 1;
          json({ project, revision: project!.organizationRevision });
          return true;
        }
        return false;
      };
    const local = await fixture("Local", false, { handle: handler("local") });
    const remote = await fixture("Remote", true, { handle: handler("remote") });
    for (const [owner, server] of [
      ["local", local],
      ["remote", remote],
    ] as const) {
      const project = server.snapshot.runtime.projects[0];
      assert.ok(project);

      Object.assign(project, {
        id: owner === "local" ? "logical-project" : "remote-project",
        path: owner === "local" ? "/same/project" : "/remote/project",
        name: owner === "local" ? "Home project" : "Remote project",
        homeServerId: owner,
        organizationRevision: owner === "local" ? 3 : 9,
        peerTeamsRevision: owner === "local" ? 4 : 8,
        folders: [{ id: "folder", name: `${owner} folder`, parentId: null }],
        ...(owner === "remote"
          ? {
              projectAliases: [
                {
                  serverId: "local",
                  projectId: "logical-project",
                  name: "Home project",
                },
              ],
              // Only the remote server reports layr support for its folder.
            }
          : {}),
      } satisfies Partial<ProjectEntityDto>);
      Object.assign(server.snapshot.threads[0]!, {
        cwd: project.path,
        pinned: true,
        projectFolderRevision: 2,
        updated: Date.now() / 1000,
      } satisfies Partial<TestAgent>);
      server.snapshot.runtime.agentSummaries = [
        agentSummaryFixture("overlap", {
          name: `${server === local ? "Local" : "Remote"} chat`,
          projectPath: project.path!,
          projectId: project.id!,
          pinned: true,
          projectFolderRevision: 2,
          updated: Date.now() / 1000,
        }),
      ];
      Object.assign(server.snapshot.runtime, {
        projectOrganizationVersion: 1,
        peerTeamsVersion: 1,
        sidebarOrder: { revision: 1, groups: {} },
      } satisfies Partial<TestRuntimeView>);
    }
    local.aliases.remote = "REM";
    local.discoverPeer(remote);
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    try {
      await remoteNetwork(context, remote);
      await page.goto(local.origin);
      if (viewport.width < 760)
        await page
          .getByRole("button", { name: "Servers and chats", exact: true })
          .click();
      const sidebar = page.locator("#sidebar");
      await expect(
        sidebar.getByRole("button", { name: /Remote chat/ }).first(),
      ).toBeVisible();
      await expect(
        sidebar.getByRole("button", { name: /Local chat/ }).first(),
      ).toBeVisible();
      await expect(sidebar.locator(".sidebar-project")).toHaveCount(1);
      const projectName = sidebar.locator(".project-tree-toggle", {
        hasText: "Home project",
      });
      await expect(projectName).toBeVisible();
      await expect(projectName).toHaveAccessibleDescription(
        "Drag to reorder. Alt + Up or Down also works.",
      );
      await expect(projectName).toHaveAttribute("aria-expanded", "true");
      expect(await projectName.evaluate((element) => element.tagName)).toBe(
        "BUTTON",
      );
      const chatCount = await sidebar.locator("[data-chat]").count();
      // The heading collapses the merged project with the chats of both servers.
      await projectName.click();
      await expect(projectName).toHaveAttribute("aria-expanded", "false");
      await expect(sidebar.locator("[data-chat]")).toHaveCount(0);
      await projectName.click();
      await expect(sidebar.locator("[data-chat]")).toHaveCount(chatCount);
      await expect(page.getByLabel("Studio server")).toHaveCount(0);
      await expect(
        sidebar.getByRole("button", { name: "Studio settings", exact: true }),
      ).toBeVisible();
      await expect(sidebar.locator("[data-folder-id]")).toHaveCount(2);
      expect(calls).toEqual({ local: [], remote: [] });
      await page.screenshot({
        path: testInfo.outputPath(`combined-${theme}.png`),
      });

      let row = sidebar
        .locator(".sidebar-row")
        .filter({ hasText: "Remote chat" });
      await row.hover();
      await row
        .getByRole("button", { name: "Actions for Remote chat", exact: true })
        .click();
      await page.getByRole("menuitem", { name: "Unpin", exact: true }).click();
      await expect(row.locator(".chat-pin")).toHaveCount(0);
      await expect.poll(() => calls.remote.length).toBe(1);
      expect(calls.remote[0]).toEqual({
        path: "/api/organization",
        body: { id: "overlap", pinned: false },
      });
      expect(local.snapshot.threads[0]!.pinned).toBe(true);

      await row.hover();
      await row
        .getByRole("button", { name: "Actions for Remote chat", exact: true })
        .click();
      await page.getByRole("menuitem", { name: "Rename", exact: true }).click();
      await row.getByLabel("Chat name").fill("Remote renamed");
      await row.getByRole("button", { name: "Save name" }).click();
      row = sidebar
        .locator(".sidebar-row")
        .filter({ hasText: "Remote renamed" });
      await expect(
        row.getByRole("button", { name: /Remote renamed/ }).first(),
      ).toBeVisible();
      expect(required(calls.remote[1]).body.id).toBe("overlap");
      expect(required(calls.remote[1]).body.request_id).toBeTruthy();
      expect(local.snapshot.threads[0]!.name).toBe("Local chat");

      await row.hover();
      await row
        .getByRole("button", {
          name: "Actions for Remote renamed",
          exact: true,
        })
        .click();
      await page
        .getByRole("menuitem", { name: "Move to folder", exact: true })
        .click();
      await page
        .getByLabel("Move to folder", { exact: true })
        .selectOption({ label: "remote folder" });
      await page
        .getByRole("button", { name: "Move chat", exact: true })
        .click();
      await expect(
        page.getByRole("dialog").filter({
          has: page.getByRole("heading", { name: "Move chat", exact: true }),
        }),
      ).toHaveCount(0);
      await expect(
        row.locator("xpath=ancestor::*[@data-folder-id][1]"),
      ).toHaveAttribute(
        "data-folder-id",
        JSON.stringify(["folder", "remote", "/remote/project", "folder"]),
      );
      expect(calls.remote[2]!.body).toEqual({
        id: "overlap",
        project_path: "/remote/project",
        project_folder: "folder",
        expected_folder: null,
        expected_revision: 2,
      });
      expect(local.snapshot.threads[0]!.projectFolder).toBeUndefined();
      expect(calls.local).toEqual([]);

      await sidebar
        .getByRole("button", { name: /remote folder Codex/ })
        .hover();
      await sidebar
        .getByRole("button", {
          name: "Options for folder remote folder",
          exact: true,
        })
        .click();
      await page
        .getByRole("menuitem", { name: "Rename folder", exact: true })
        .click();
      await expect(
        page
          .getByRole("dialog")
          .filter({
            has: page.getByRole("heading", {
              name: "Rename folder",
              exact: true,
            }),
          })
          .locator(".project-directory"),
      ).toHaveText("/remote/project");
      await page
        .getByRole("textbox", { name: "Folder name", exact: true })
        .fill("Remote folder renamed");
      await page
        .getByRole("button", { name: "Save name", exact: true })
        .click();
      await expect(
        page.getByRole("dialog").filter({
          has: page.getByRole("heading", {
            name: "Rename folder",
            exact: true,
          }),
        }),
      ).toHaveCount(0);
      expect(calls.remote[3]!.body).toMatchObject({
        action: "rename_folder",
        path: "/remote/project",
        folder_id: "folder",
        expected_revision: 9,
      });
      expect(local.snapshot.runtime.projects[0]!.folders![0]!.name).toBe(
        "local folder",
      );
      await expect(
        sidebar.getByRole("button", { name: /Remote folder renamed Codex/ }),
      ).toBeVisible();

      await row
        .getByRole("button", { name: /Remote renamed/ })
        .first()
        .click();
      await expect(
        page.locator('iframe[title="Studio on Remote"]'),
      ).toBeVisible();
      if (viewport.width < 760)
        await page
          .getByRole("button", { name: "Servers and chats", exact: true })
          .click();
      await sidebar
        .getByLabel("Filter projects and chats")
        .fill("/remote/project");
      await expect(row).toBeVisible();
      await expect(
        sidebar.locator(".sidebar-row").filter({ hasText: "Local chat" }),
      ).toHaveCount(0);
      await sidebar.getByLabel("Filter projects and chats").fill("");
      remote.offline(true);
      await expect(async () => {
        await row.hover();
        await row
          .getByRole("button", {
            name: "Actions for Remote renamed",
            exact: true,
          })
          .click();
        await expect(
          page.getByRole("menuitem", { name: "Pin", exact: true }),
        ).toHaveAttribute("data-disabled", "true");
        await page.keyboard.press("Escape");
      }).toPass({ timeout: 15000 });
      await expect(row).toBeVisible();
      expect(calls.local).toEqual([]);

      expect(calls.local).toEqual([]);
      expect(errors).toEqual([]);
      await testInfo.attach("owner-requests", {
        body: JSON.stringify(calls, null, 2),
        contentType: "application/json",
      });
    } finally {
      await remote.close();
      await local.close();
    }
  });
}

test("offline folder expansion preserves the saved source state after reconnect", async ({
  page,
  context,
}, testInfo) => {
  test.setTimeout(60000);
  await page.setViewportSize({ width: 1440, height: 900 });
  const preferenceWrites = [];
  const local = await fixture("Local");
  const remote = await fixture("Remote", true, {
    handle: ({ request, url, body }) => {
      if (request.method === "POST" && url.pathname === "/api/sync/preferences")
        preferenceWrites.push(body);
      return false;
    },
  });
  const path = "/remote/project";
  const key = ":server:remote:codex-project-tree:/same/state";
  const saved = { [JSON.stringify([path, "folder", "folder"])]: true };
  Object.assign(remote.snapshot.runtime.projects[0]!, {
    path,
    name: "Remote project",
    folders: [{ id: "folder", name: "Saved closed folder" }],
  } satisfies Partial<ProjectEntityDto>);
  Object.assign(remote.snapshot.threads[0]!, {
    cwd: path,
    projectFolder: "folder",
    updated: Date.now() / 1000,
  } satisfies Partial<TestAgent>);
  remote.snapshot.runtime.agentSummaries = [
    agentSummaryFixture("overlap", {
      name: "Remote chat",
      projectPath: path,
      projectFolder: "folder",
      updated: Date.now() / 1000,
    }),
  ];
  Object.assign(remote.snapshot.runtime, {
    projectOrganizationVersion: 1,
    sidebarOrder: { revision: 1, groups: {} },
  } satisfies Partial<TestRuntimeView>);
  local.discoverPeer(remote);
  try {
    await context.addInitScript(
      ({ origin, destination, key, saved }) => {
        if (
          new URLSearchParams(location.search).get("studio-server") === "remote"
        )
          localStorage.setItem(key, JSON.stringify(saved));
        const nativeFetch = window.fetch.bind(window);
        window.fetch = async (input, init) => {
          const request = new Request(input, init);
          const url = new URL(request.url);
          if (url.origin !== origin) return nativeFetch(request);
          const body = await request.clone().arrayBuffer();
          return nativeFetch(destination + url.pathname + url.search, {
            method: request.method,
            headers: request.headers,
            ...(body.byteLength ? { body } : {}),
            signal: request.signal,
            redirect: "error",
            credentials: "omit",
          });
        };
      },
      {
        origin: remote.invitation.origin,
        destination: remote.origin,
        key,
        saved,
      },
    );
    await page.goto(local.origin);
    const sidebar = page.locator("#sidebar");
    const folder = sidebar.getByRole("button", {
      name: "Saved closed folder",
      exact: true,
    });
    await expect(folder).toHaveAttribute("aria-expanded", "false");
    const remoteFrame = page.frameLocator('iframe[title="Studio on Remote"]');
    const original = await remoteFrame
      .locator("html")
      .evaluate((_, key) => localStorage.getItem(key), key);
    expect(JSON.parse(original!)).toEqual(saved);
    const writesBefore = preferenceWrites.length;
    remote.offline(true);
    const newChat = sidebar.getByRole("button", {
      name: "New chat in folder Saved closed folder",
      exact: true,
    });
    await expect(newChat).toBeDisabled();
    await folder.click();
    await expect(folder).toHaveAttribute("aria-expanded", "true");
    await expect(
      sidebar.locator(".sidebar-row").filter({ hasText: "Remote chat" }),
    ).toBeVisible();
    await page.screenshot({
      path: testInfo.outputPath("offline-expanded.png"),
    });
    expect(
      await remoteFrame
        .locator("html")
        .evaluate((_, key) => localStorage.getItem(key), key),
    ).toBe(original);
    remote.offline(false);
    remote.complete("fresh-after-reconnect");
    await expect(newChat).toBeEnabled({ timeout: 20000 });
    await expect(folder).toHaveAttribute("aria-expanded", "false", {
      timeout: 20000,
    });
    expect(
      await remoteFrame
        .locator("html")
        .evaluate((_, key) => localStorage.getItem(key), key),
    ).toBe(original);
    expect(preferenceWrites.length).toBe(writesBefore);
    expect(remote.writes).toEqual([]);
    expect(local.writes).toEqual([]);
  } finally {
    await remote.close();
    await local.close();
  }
});

declare global {
  interface Window {
    __sidebarReturnedFocus: boolean;
  }
}
