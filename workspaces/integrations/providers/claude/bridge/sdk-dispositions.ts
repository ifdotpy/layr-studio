import type { BridgeOptions } from "./bridge.ts";
import { PERSISTENT_UNSUPPORTED_REASON } from "./settings-policy.generated.ts";
import type { Options, Query } from "@anthropic-ai/claude-agent-sdk";
import { readFileSync } from "node:fs";
import { z } from "zod";

// Pinned by package.json; additions require a disposition before tsc passes.
export const CLAUDE_SDK_VERSION = z
  .object({
    dependencies: z.object({ "@anthropic-ai/claude-agent-sdk": z.string() }),
  })
  .parse(
    JSON.parse(readFileSync(new URL("package.json", import.meta.url), "utf8")),
  ).dependencies["@anthropic-ai/claude-agent-sdk"];
export type Disposition =
  | { kind: "Handled" }
  | { kind: "Ignored"; reason: string };
const handled = { kind: "Handled" } as const;
const ignored = (reason: string) => ({ kind: "Ignored" as const, reason });

type ConfiguredOptionKeys<T> = T extends unknown ? keyof T : never;

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
} satisfies {
  [K in keyof Options]-?: K extends ConfiguredOptionKeys<BridgeOptions>
    ? Extract<Disposition, { kind: "Handled" }>
    : Extract<Disposition, { kind: "Ignored" }>;
};

// This native invocation surface is the support owner. Bind lazily available
// members independently so one unavailable control cannot prevent query startup.
export function querySurface(q: Query) {
  const nativeIterator = q[Symbol.asyncIterator]();
  const iterator = {
    next: nativeIterator.next.bind(nativeIterator),
    return: nativeIterator.return.bind(nativeIterator),
  };
  return {
    ...iterator,
    interrupt: q.interrupt?.bind(q),
    setPermissionMode: q.setPermissionMode?.bind(q),
    setModel: q.setModel?.bind(q),
    applyFlagSettings: q.applyFlagSettings?.bind(q),
    initializationResult: q.initializationResult?.bind(q),
    reinitialize: q.reinitialize?.bind(q),
    supportedCommands: q.supportedCommands?.bind(q),
    supportedModels: q.supportedModels?.bind(q),
    mcpServerStatus: q.mcpServerStatus?.bind(q),
    getContextUsage: q.getContextUsage?.bind(q),
    usage_EXPERIMENTAL_MAY_CHANGE_DO_NOT_RELY_ON_THIS_API_YET:
      q.usage_EXPERIMENTAL_MAY_CHANGE_DO_NOT_RELY_ON_THIS_API_YET?.bind(q),
    accountInfo: q.accountInfo?.bind(q),
    reconnectMcpServer: q.reconnectMcpServer?.bind(q),
    stopTask: q.stopTask?.bind(q),
    backgroundTasks: q.backgroundTasks?.bind(q),
    close: q.close?.bind(q),
    [Symbol.asyncIterator]: () => iterator,
  };
}
export type BridgeQuery = ReturnType<typeof querySurface>;

export const sdkQueryUnsupported = {
  setMcpServers: ignored(
    "Live Studio server replacement requires destructive removal; reopen the query to change tools.",
  ),
  setMcpPermissionModeOverride: ignored(
    "No per-server permission override control is exposed.",
  ),
  setMaxThinkingTokens: ignored(
    "Thinking uses session flags and the thinking option.",
  ),
  updateSettings: ignored(PERSISTENT_UNSUPPORTED_REASON),
  supportedAgents: ignored("Studio owns the agent catalog."),
  readFile: ignored("File operations use normal native tool permissions."),
  reloadPlugins: ignored("No live native plugin reload control is exposed."),
  reloadSkills: ignored("No live native skill reload control is exposed."),
  reloadOutputStyles: ignored(
    "No live output style reload control is exposed.",
  ),
  rewindFiles: ignored("Studio restores verified file checkpoints itself."),
  seedReadState: ignored("Read provenance remains SDK-owned."),
  toggleMcpServer: ignored("MCP changes use verified full catalogs."),
  readMcpResource: ignored("MCP resources are invoked through native tools."),
  streamInput: ignored("The query prompt async iterator owns input delivery."),
  throw: ignored("Studio does not inject exceptions into the native iterator."),
  [Symbol.asyncDispose]: ignored(
    "Queries are released through close and iterator return.",
  ),
} satisfies Record<
  Exclude<keyof Query, keyof BridgeQuery>,
  Extract<Disposition, { kind: "Ignored" }>
>;
