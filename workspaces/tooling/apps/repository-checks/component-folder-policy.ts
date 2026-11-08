// Owner: exceptions to structural component ownership; paths are relative to web/src.
// Product imports stay within their support grouping unless a shared API is named here.
export type ComponentFolderPolicy = {
  sharedSupport: Readonly<Record<string, string>>;
  multiple: Readonly<Record<string, string>>;
  cssOrderOwner?: string;
};
export const componentFolderPolicy: ComponentFolderPolicy = {
  sharedSupport: {
    "components/ui/support":
      "UI foundation hooks and interaction helpers serve every feature.",
    "components/accounts/usage/support":
      "Account usage views share limit colors with account selection surfaces.",
    "components/accounts/support":
      "Account selection contracts are shared with usage and role settings.",
    "components/sidebar/chat-row/support":
      "Sidebar row action contracts are shared with row and project surfaces.",
    "components/sidebar/chat-status/support":
      "Sidebar activity facts are also used by agent and transcript status views.",
    "components/sidebar/support":
      "App and server sidebar owners share the sidebar model and service contracts.",
    "components/agents/support":
      "Agent and team contexts are composed by the application shell and model pickers.",
    "components/settings/models/support":
      "Model selection contracts serve account, agent, and conversation settings.",
    "components/conversation/support":
      "Transcript and request contexts are shared with app lifecycle owners and message panels.",
    "components/shell/tasks/support/backgroundTaskModel.ts":
      "Task summary facts also drive sidebar activity indicators.",
  },
  multiple: {
    "components/agents/AgentPhase/AgentPhase.tsx":
      "exports share private leaves",
    "components/agents/AgentProgress/AgentProgress.tsx":
      "exports share private leaves",
    "components/agents/ExecutionSettings/ExecutionSettings.tsx":
      "exports share private leaves",
    "components/agents/TeamRetainedWorktrees/TeamRetainedWorktrees.tsx":
      "exports share private leaves",
    "components/agents/UnifiedAgentSettings/UnifiedAgentSettings.tsx":
      "not split: network boundary exception pending owner migration",
    "components/conversation/ComposerAttachments/ComposerAttachments.tsx":
      "not split: network boundary exception pending owner migration",
    "components/conversation/markdown/NativeMarkdown/NativeMarkdown.tsx":
      "exports share private leaves",
    "components/conversation/prompt-composer/PromptComposer/PromptComposer.tsx":
      "exports share private leaves",
    "components/conversation/transcript/Activity/Activity.tsx":
      "exports share private leaves",
    "components/servers/MultiServerApp/MultiServerApp.tsx":
      "not split: network boundary exception pending owner migration",
    "components/servers/ServerServicesProvider/ServerServicesProvider.tsx":
      "not split: network boundary exception pending owner migration",
    "components/settings/HotkeySettings/HotkeySettings.tsx":
      "exports share private leaves",
    "components/shell/NativeNotice/NativeNotice.tsx":
      "exports share private leaves",
    "components/shell/ServerUpdateStatus/ServerUpdateStatus.tsx":
      "exports share private leaves",
    "components/shell/TerminalDock/TerminalDock.tsx":
      "exports share private leaves",
    "components/shell/messages/PeerTeamGroup/PeerTeamGroup.tsx":
      "not split: network boundary exception pending owner migration",
    "components/shell/tasks/TaskElapsed/TaskElapsed.tsx":
      "exports share private leaves",
    "components/sidebar/SidebarWarnings/SidebarWarnings.tsx":
      "exports share private leaves",
    "components/ui/PanelHeader/PanelHeader.tsx": "exports share private leaves",
  },
  cssOrderOwner: "rendererStyles.ts",
};
