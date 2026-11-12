import type { Options, Query } from "@anthropic-ai/claude-agent-sdk";

// Pinned by package.json; additions require a disposition before tsc passes.
export const CLAUDE_SDK_VERSION = "0.3.308";
export type Disposition =
  | { kind: "Handled" }
  | { kind: "Ignored"; reason: string };
const handled = { kind: "Handled" } as const;
const ignored = (reason: string): Disposition => ({ kind: "Ignored", reason });

export const sdkOptionsDispositions = {
  // Preparation queries have bounded cancellation.
  abortController: handled,
  additionalDirectories: ignored(
    "Studio scopes sessions to one project folder.",
  ),
  projectConfigRoot: ignored(
    "The SDK resolves project configuration from cwd.",
  ),
  agent: ignored("Studio owns agent roles and instructions."),
  agents: ignored("Studio manages subagents through orchestration."),
  allowedTools: handled,
  // Native tools await Studio permission callbacks.
  canUseTool: handled,
  continue: ignored("Resume always uses an exact native session identity."),
  // Every query is scoped to the Studio project folder.
  cwd: handled,
  // Studio orchestrates agents instead of native Agent tools.
  disallowedTools: handled,
  toolAliases: ignored("Studio does not rename native tools."),
  // Ephemeral and moved queries retain their tool catalog.
  tools: handled,
  // Queries receive the isolated session environment.
  env: handled,
  executable: ignored("Use the SDK default Node executable."),
  executableArgs: ignored("No extra SDK host arguments are required."),
  // Preserve user-message replay and account launch flags.
  extraArgs: handled,
  fallbackModel: ignored("Model changes are explicit account choices."),
  // Native file checkpoints support edit and recovery.
  enableFileCheckpointing: handled,
  toolConfig: ignored("No native tool-specific configuration UI exists."),
  forkSession: ignored("Forks use the SDK session API before query startup."),
  betas: ignored("No experimental API beta is enabled implicitly."),
  // SessionStart guards native identity transitions.
  hooks: handled,
  // MCP elicitation waits for the Studio user.
  onElicitation: handled,
  // Resume dialogs use Studio questions.
  onUserDialog: handled,
  // Declare the supported resume_return dialog.
  supportedDialogKinds: handled,
  // Expose task Stop controls.
  perTaskStopAffordance: handled,
  // Ephemeral queries disable native persistence.
  persistSession: handled,
  sessionStore: ignored("Use the native SDK transcript store."),
  sessionStoreFlush: ignored("No custom session store is installed."),
  loadTimeoutMs: ignored("Preparation has a shared bridge deadline."),
  includeHookEvents: ignored("SessionStart callbacks own transition evidence."),
  // Stream deltas drive text and usage updates.
  includePartialMessages: handled,
  forwardSubagentText: ignored(
    "Subagent messages retain their native parent identity.",
  ),
  verbatimPrompts: ignored("Preserve normal SDK prompt construction."),
  thinking: ignored(
    "Thinking is applied through session flag settings; direct query thinking is not exposed.",
  ),
  // Use the requested model reasoning effort.
  effort: handled,
  maxThinkingTokens: ignored("Thinking uses the current thinking option."),
  maxTurns: ignored("Studio owns turn lifecycle without a hidden turn cap."),
  maxBudgetUsd: ignored("Studio does not enforce a native monetary budget."),
  taskBudget: ignored("Studio does not expose native task budgets."),
  // Provide Studio tools and verified external servers.
  mcpServers: handled,
  // Use the selected account model.
  model: handled,
  outputFormat: ignored("Studio consumes ordinary assistant/tool messages."),
  // Use the selected native CLI.
  pathToClaudeCodeExecutable: handled,
  // Apply the session permission policy.
  permissionMode: handled,
  planModeInstructions: ignored(
    "Studio supplies common developer instructions.",
  ),
  // Permit explicitly selected full access.
  allowDangerouslySkipPermissions: handled,
  permissionPromptToolName: ignored(
    "Permission prompts use the typed callback.",
  ),
  permissionPrompts: ignored("Studio owns the approval UI."),
  plugins: ignored("User configuration resolves native plugins."),
  pluginDelivery: ignored("Native plugin delivery remains SDK-owned."),
  promptSuggestions: ignored("Studio does not present prompt suggestions."),
  agentProgressSummaries: ignored(
    "Studio owns orchestrator progress messages.",
  ),
  // Resume the confirmed native session.
  resume: handled,
  // Start with the durable native session identity.
  sessionId: handled,
  resumeSessionAt: ignored(
    "History controls use verified native fork boundaries.",
  ),
  resumeDropsTurn: ignored("History controls use explicit native boundaries."),
  // Native reviewers use isolated SDK sandbox settings.
  sandbox: handled,
  // Apply validated session flag overlays.
  settings: handled,
  managedSettings: ignored("Host policy remains native managed configuration."),
  // Use local settings or verified move snapshots.
  settingSources: handled,
  skills: ignored("Native skills are discovered through configuration."),
  debug: ignored("Native debug mode is not enabled implicitly."),
  debugFile: ignored("Diagnostics use the existing stderr path."),
  // Route CLI diagnostics to bridge stderr.
  stderr: handled,
  // Managed and moved sessions restrict external MCP.
  strictMcpConfig: handled,
  // Use Studio instructions and verified prompt snapshots.
  systemPrompt: handled,
  title: ignored("Studio stores thread names separately."),
  spawnClaudeCodeProcess: ignored("Use the SDK process transport."),
} satisfies Record<keyof Options, Disposition>;

export const sdkQueryDispositions = {
  // Stop and recovery interrupt the native query.
  interrupt: handled,
  // Apply live permission changes.
  setPermissionMode: handled,
  setMcpPermissionModeOverride: ignored(
    "No per-server permission override control is exposed.",
  ),
  // Apply live model changes.
  setModel: handled,
  setMaxThinkingTokens: ignored(
    "Thinking uses session flags and the thinking option.",
  ),
  // Apply current session flag settings.
  applyFlagSettings: handled,
  updateSettings: ignored("Studio does not write native user settings files."),
  // Read native capabilities and commands.
  initializationResult: handled,
  // Recover controls and verified account evidence.
  reinitialize: handled,
  supportedCommands: ignored(
    "The initialization result already includes commands.",
  ),
  // Read the account model catalog.
  supportedModels: handled,
  supportedAgents: ignored("Studio owns the agent catalog."),
  // Read and verify external MCP catalogs.
  mcpServerStatus: handled,
  // Main-loop occupancy is read at result boundaries, separately from billing.
  getContextUsage: handled,
  // Read account plan usage.
  usage_EXPERIMENTAL_MAY_CHANGE_DO_NOT_RELY_ON_THIS_API_YET: handled,
  readFile: ignored("File operations use normal native tool permissions."),
  reloadPlugins: ignored("No live native plugin reload control is exposed."),
  reloadSkills: ignored("No live native skill reload control is exposed."),
  reloadOutputStyles: ignored(
    "No live output style reload control is exposed.",
  ),
  // Verify native account identity.
  accountInfo: handled,
  rewindFiles: ignored("Studio restores verified file checkpoints itself."),
  seedReadState: ignored("Read provenance remains SDK-owned."),
  // Reconnect pending or requested MCP servers.
  reconnectMcpServer: handled,
  toggleMcpServer: ignored("MCP changes use verified full catalogs."),
  readMcpResource: ignored("MCP resources are invoked through native tools."),
  // Apply verified live external MCP configurations.
  setMcpServers: handled,
  streamInput: ignored("The query prompt async iterator owns input delivery."),
  // Stop explicit or policy-selected background tasks.
  stopTask: handled,
  // Move foreground tools to background.
  backgroundTasks: handled,
  // Release native process ownership.
  close: handled,
  next: handled,
  return: handled,
  throw: handled,
  [Symbol.asyncIterator]: handled,
  [Symbol.asyncDispose]: handled,
} satisfies Record<keyof Query, Disposition>;
