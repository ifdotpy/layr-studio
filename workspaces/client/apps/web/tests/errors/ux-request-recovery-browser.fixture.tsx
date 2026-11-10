import "../../src/rendererStyles.ts";
import { AgentModelsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { Agent, Project } from "../../src/types.ts";
import type { GetResult } from "../../src/apiTypes.ts";
declare global {
  interface Window {
    selected: string[];
    saves: number;
    failRefresh: boolean;
  }
}

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import Picker from "../../src/components/projects/ProjectDirectoryPicker/ProjectDirectoryPicker.tsx";
import { ClaudeSettings } from "../../src/components/settings/ClaudeSettings/ClaudeSettings.tsx";
import { ProjectNameForm } from "../../src/components/projects/ProjectNameForm/ProjectNameForm.tsx";
import { MoveChatForm } from "../../src/components/projects/MoveChatForm/MoveChatForm.tsx";
import ProjectAccount from "../../src/components/projects/ProjectAccount/ProjectAccount.tsx";
import { AccountTransferStatus } from "../../src/components/accounts/AccountTransferStatus/AccountTransferStatus.tsx";
const mode = new URLSearchParams(location.search).get("case")!;
const project: Project = {
  id: "/project",
  path: "/project",
  name: "Project",
  accountKey: "ready",
  accountKeys: ["ready", "expired"],
  folders: [{ id: "review", name: "Review" }],
};
const agent: Agent = {
  id: "lead",
  rootId: "lead",
  name: "Lead",
  provider: "claude",
  accountKey: "ready",
  status: "idle",
  model: "claude-opus-4-6",
  projectFolder: null,
  projectFolderRevision: 0,
};
const account = (
  id: string,
  label: string,
  status: GetResult<"/api/accounts">["accounts"][number]["status"],
): GetResult<"/api/accounts">["accounts"][number] => ({
  id,
  label,
  status,
  provider: "codex",
  home: "/home/fixture/" + id,
  source: "Codex CLI",
});
const accounts: GetResult<"/api/accounts"> = {
  accounts: [
    account("ready", "Ready account", "ready"),
    account("expired", "Expired account", "signedOut"),
  ],
  archivedAccounts: [],
  logins: [],
  supportsDisconnect: true,
  supportsDelete: true,
  defaultAccountGroups: [],
  defaultAccountKey: "ready",
};
window.selected = [];
window.saves = 0;
window.failRefresh = mode === "confirmed";
const saved = async () => {
  window.saves++;
  if (window.failRefresh) {
    window.failRefresh = false;
    throw Error("Snapshot unavailable");
  }
};
let body =
  mode === "picker" ? (
    <Picker
      initialPath="/project"
      onSelect={async (path) => {
        window.selected.push(path);
      }}
    />
  ) : mode === "claude" || mode === "deadline" ? (
    <ClaudeSettings agent={agent} />
  ) : ["name", "confirmed", "rejected", "project-deadline"].includes(mode) ? (
    <ProjectNameForm project={project} folder="new" saved={saved} />
  ) : mode === "move" ? (
    <MoveChatForm project={project} agent={agent} saved={saved} />
  ) : mode === "project-account" || mode === "unready" ? (
    <ProjectAccount
      path={project.path!}
      project={
        mode === "unready" ? { ...project, accountKey: "expired" } : project
      }
      defaultAccountKey="ready"
      accounts={accounts}
      saved={saved}
    />
  ) : (
    <AccountTransferStatus
      transfer={{
        kind: "waiting",
        id: "fixture-transfer",
        targetAccountKey: "ready",
        reason: "Waiting for the next turn",
        actions: "none",
      }}
      targetLabel="Ready account"
      onAction={() => {}}
    />
  );
createRoot(document.getElementById("root")!).render(
  <MantineProvider>
    <AgentModelsFixture agents={[agent]}>{body}</AgentModelsFixture>
  </MantineProvider>,
);
