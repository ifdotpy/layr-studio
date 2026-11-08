import { WorkspaceMode } from "./generated/api.ts";
import { expect, test } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { SignInPushSettings } from "./components/accounts/SignInPushSettings/SignInPushSettings.tsx";
import { nativeErrorView } from "./nativeErrors.ts";
import { workspaceBadgeInfo } from "./components/shell/workspace/support/WorkspaceBadge.tsx";
import { resultTarget } from "./components/conversation/questions/SignInCard/signInCardModel.ts";
import { deviceName } from "./push/signInPush.ts";
import type { Agent } from "./types.ts";

test("unsupported push alerts give platform-neutral setup guidance", () => {
  const markup = renderToStaticMarkup(
    createElement(SignInPushSettings, { active: true }),
  );
  expect(markup).toContain("This window cannot get push alerts.");
  expect(markup).toContain(
    "Try adding Studio to your home screen and opening it from there.",
  );
  expect(markup).not.toContain("iPhone");
});

test("disk recovery, folder sync, workspaces and sign-in use platform-neutral product words", () => {
  expect(nativeErrorView("ENOSPC: no space left on device").hint).toBe(
    "Free disk space on the server. Queued work starts again on its own.",
  );
  expect(
    workspaceBadgeInfo({
      isLead: true,
      workspaceMode: WorkspaceMode.image,
    } as Agent),
  ).toBeNull();
  expect(
    workspaceBadgeInfo({
      isLead: true,
      workspaceMode: WorkspaceMode.layr,
    } as Agent),
  ).toEqual({ label: "Unavailable", title: "Legacy workspace unavailable" });
  expect(
    resultTarget(
      {
        host: "id.example.com",
        url: "https://id.example.com/",
        port: 8000,
      },
      "Build server",
    ),
  ).toBe("Build server, port 8000");
  expect(deviceName("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)")).toBe(
    "Computer",
  );
});

test("user-supplied server labels and raw error details stay intact", () => {
  const details = "ENOENT: /projects/Linux tools/Mac notes.txt";
  expect(nativeErrorView(details).message).toBe(details);
  expect(
    resultTarget(
      {
        host: "id.example.com",
        url: "https://id.example.com/",
        port: 8000,
      },
      "Linux build server",
    ),
  ).toBe("Linux build server, port 8000");
});
