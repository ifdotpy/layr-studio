# Temporary HTTP ingress audit

Generated from OpenAPI and route registrations on the combined query branch. `review` / `unverified` means manual inspection is required; a bounded root still needs nested-field review. A marker only says middleware skips that part, not that the type is strict. Forwarded identity bypasses the normal handler and needs a separate audit.

| Method and route                            | Part  | Handler input                                                         | deny_unknown_fields | Bounds                  | Middleware skip marker | Route file / type owner                     |
| ------------------------------------------- | ----- | --------------------------------------------------------------------- | ------------------- | ----------------------- | ---------------------- | ------------------------------------------- |
| `POST /api/accounts/claude/add`             | body  | `Add`                                                                 | no                  | review                  | no                     | claude_login.rs                             |
| `GET /api/accounts/claude/login`            | query | `LoginStatusQuery`                                                    | yes                 | review                  | yes                    | claude_login.rs                             |
| `POST /api/accounts/claude/login`           | body  | `Start`                                                               | no                  | review                  | no                     | claude_login.rs                             |
| `POST /api/accounts/claude/login/cancel`    | body  | `Cancel`                                                              | no                  | review                  | no                     | claude_login.rs                             |
| `POST /api/accounts/claude/login/code`      | body  | `Code`                                                                | no                  | review                  | no                     | claude_login.rs                             |
| `POST /api/accounts/default`                | body  | `AccountKeyBody`                                                      | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/accounts/default-groups`         | body  | `DefaultGroupsBody`                                                   | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/accounts/delete`                 | body  | `DeleteAccountBody`                                                   | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/accounts/disconnect`             | body  | `RequiredAccountKeyBody`                                              | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/accounts/discover`               | body  | `none / manual`                                                       | unverified          | unverified              | no                     | account_login.rs                            |
| `POST /api/accounts/group`                  | body  | `AccountGroupBody`                                                    | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/accounts/login`                  | body  | `LoginRequest`                                                        | unverified          | unverified              | no                     | account_login.rs                            |
| `POST /api/accounts/login/cancel`           | body  | `CancelLoginBody`                                                     | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/accounts/name`                   | body  | `AccountNameBody`                                                     | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/accounts/reconnect`              | body  | `RequiredAccountKeyBody`                                              | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/accounts/register`               | body  | `RegisterAccountBody`                                                 | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/accounts/throttle`               | body  | `StartThrottleBody`                                                   | no                  | review                  | no                     | account_login.rs                            |
| `POST /api/action`                          | body  | `generated::NativeActionRequest`                                      | unverified          | unverified              | no                     | agent_management.rs                         |
| `GET /api/agent-chat`                       | query | `AgentChatQuery`                                                      | yes                 | bounded / review nested | yes                    | messages.rs                                 |
| `POST /api/agents`                          | body  | `generated::CreateAgentRequest`                                       | unverified          | unverified              | no                     | agents_create.rs                            |
| `POST /api/agents/account`                  | body  | `generated::AccountSelectionRequest`                                  | unverified          | unverified              | no                     | account_transfer.rs                         |
| `POST /api/agents/account-transfer`         | body  | `Request`                                                             | unverified          | unverified              | no                     | account_transfer.rs                         |
| `GET /api/agents/finder-view`               | query | `FinderQuery`                                                         | yes                 | bounded / review nested | yes                    | catalog_agent.rs                            |
| `POST /api/agents/move-to-host`             | body  | `generated::AgentMoveToHostRequest`                                   | unverified          | unverified              | no                     | agents_create.rs                            |
| `POST /api/agents/workspace-mode`           | body  | `generated::AgentWorkspaceModeRequest`                                | unverified          | unverified              | no                     | agents_create.rs                            |
| `GET /api/analytics`                        | query | `AnalyticsQuery`                                                      | yes                 | review                  | yes                    | insights.rs                                 |
| `POST /api/annotation`                      | body  | `AnnotationBody`                                                      | yes                 | review                  | no                     | work_items.rs                               |
| `POST /api/answer`                          | body  | `crate::generated::AnswerBody`                                        | unverified          | unverified              | no                     | questions_write.rs                          |
| `POST /api/assets`                          | body  | `crate::generated::AssetUpload`                                       | unverified          | unverified              | no                     | workspace_files.rs                          |
| `POST /api/branch`                          | body  | `crate::generated::BranchRequest`                                     | unverified          | unverified              | no                     | branches.rs                                 |
| `POST /api/browser/forward`                 | body  | `ForwardInput`                                                        | unverified          | unverified              | no                     | browser.rs                                  |
| `GET /api/capabilities`                     | query | `CapabilitiesQuery`                                                   | yes                 | review                  | yes                    | catalog_agent.rs                            |
| `POST /api/capacity-retry`                  | body  | `crate::generated::CapacityRetryRequest`                              | unverified          | unverified              | no                     | usage.rs                                    |
| `GET /api/changes`                          | query | `ChangesQuery`                                                        | yes                 | review                  | yes                    | workspace_views.rs                          |
| `POST /api/checkpoint`                      | body  | `crate::generated::CheckpointCaptureRequest`                          | unverified          | unverified              | no                     | checkpoints.rs                              |
| `POST /api/checkpoint/preview`              | body  | `crate::generated::CheckpointPreviewRequest`                          | unverified          | unverified              | no                     | checkpoints.rs                              |
| `POST /api/checkpoint/restore`              | body  | `crate::generated::CheckpointRestoreRequest`                          | unverified          | unverified              | no                     | checkpoints.rs                              |
| `GET /api/checkpoints`                      | query | `CheckpointsQuery`                                                    | yes                 | review                  | yes                    | questions_reads.rs                          |
| `POST /api/claude/file-backups`             | body  | `crate::generated::ClaudeFileBackupsRequest`                          | unverified          | unverified              | no                     | claude_controls.rs                          |
| `POST /api/claude/profiles`                 | body  | `ProfileRequest`                                                      | unverified          | unverified              | no                     | account_login.rs                            |
| `GET /api/claude/repair-report`             | query | `RepairReportQuery`                                                   | yes                 | review                  | yes                    | claude_controls.rs                          |
| `POST /api/claude/session`                  | body  | `SessionRequest`                                                      | no                  | bounded / review nested | no                     | claude_controls.rs                          |
| `POST /api/commands`                        | body  | `CommandRequest`                                                      | no                  | review                  | yes                    | commands.rs → command.rs                    |
| `GET /api/complaint`                        | query | `ComplaintQuery`                                                      | yes                 | review                  | yes                    | questions_reads.rs                          |
| `POST /api/complaints`                      | body  | `crate::generated::ComplaintBody`                                     | unverified          | unverified              | no                     | questions_write.rs                          |
| `POST /api/configure`                       | body  | `generated::ConfigureRequest`                                         | unverified          | unverified              | no                     | agents_create.rs                            |
| `POST /api/connection-recovery`             | body  | `crate::generated::IdRequest`                                         | unverified          | unverified              | no                     | recovery.rs                                 |
| `POST /api/conversation`                    | body  | `generated::ConversationRequest`                                      | unverified          | unverified              | no                     | agents_create.rs                            |
| `POST /api/conversation/delete`             | body  | `generated::IdRequest`                                                | unverified          | unverified              | no                     | agent_management.rs                         |
| `GET /api/costs`                            | query | `CostsQuery`                                                          | yes                 | review                  | yes                    | usage.rs                                    |
| `POST /api/costs/interest`                  | body  | `crate::generated::CostInterestRequest`                               | unverified          | unverified              | no                     | usage.rs                                    |
| `GET /api/desktop`                          | query | `none / manual`                                                       | unverified          | unverified              | no                     | desktop.rs                                  |
| `GET /api/diagnostics/usage-reconciliation` | query | `ReconcileQuery`                                                      | yes                 | bounded / review nested | yes                    | usage_reconciliation.rs                     |
| `GET /api/directories`                      | query | `DirectoriesQuery`                                                    | yes                 | review                  | yes                    | system_reads.rs                             |
| `POST /api/federation`                      | body  | `studio_runtime_schema::federation_ingress::FederationRequest`        | yes                 | review                  | no                     | federation.rs → federation_ingress.rs       |
| `POST /api/federation/v1/message`           | body  | `none / manual`                                                       | unverified          | unverified              | no                     | federation.rs                               |
| `POST /api/federation/v1/pair`              | body  | `none / manual`                                                       | unverified          | unverified              | no                     | federation.rs                               |
| `POST /api/federation/v1/pull`              | body  | `none / manual`                                                       | unverified          | unverified              | no                     | federation.rs                               |
| `POST /api/federation/v1/status`            | body  | `none / manual`                                                       | unverified          | unverified              | no                     | federation.rs                               |
| `GET /api/file`                             | query | `FileQuery`                                                           | yes                 | review                  | yes                    | workspace_files.rs                          |
| `GET /api/file-info`                        | query | `FileQuery`                                                           | yes                 | review                  | yes                    | workspace_files.rs                          |
| `POST /api/leads`                           | body  | `generated::CreateLeadRequest`                                        | unverified          | unverified              | no                     | agents_create.rs                            |
| `GET /api/limits`                           | query | `LimitsQuery`                                                         | yes                 | review                  | yes                    | catalog.rs                                  |
| `POST /api/limits/reset`                    | body  | `studio_runtime_schema::limit_reset_request::ResetRequest`            | yes                 | review                  | yes                    | usage.rs → limit_reset_request.rs           |
| `POST /api/linux-vm/settings`               | body  | `generated::LinuxVMSettings`                                          | unverified          | unverified              | no                     | linux_vm_settings.rs                        |
| `POST /api/mac-sync/config`                 | body  | `generated::MacSyncConfigRequest`                                     | unverified          | unverified              | no                     | mac_sync.rs                                 |
| `POST /api/mac-sync/resolve`                | body  | `generated::MacSyncResolveRequest`                                    | unverified          | unverified              | no                     | mac_sync.rs                                 |
| `POST /api/mcp/reconnect`                   | body  | `crate::generated::McpReconnectRequest`                               | unverified          | unverified              | no                     | mcp_reconnect.rs                            |
| `GET /api/messages`                         | query | `MessagesQuery`                                                       | yes                 | bounded / review nested | yes                    | messages.rs                                 |
| `GET /api/messages/receipts`                | query | `ReceiptQuery`                                                        | unverified          | unverified              | yes                    | messages.rs                                 |
| `GET /api/models`                           | query | `ModelsQuery`                                                         | yes                 | review                  | yes                    | models.rs                                   |
| `POST /api/monitor/input`                   | body  | `MonitorInputRequest`                                                 | no                  | review                  | no                     | monitor_runtime.rs                          |
| `GET /api/monitor/log`                      | query | `LogQuery`                                                            | yes                 | review                  | yes                    | monitors.rs                                 |
| `POST /api/monitor/restart`                 | body  | `monitor_runtime::RestartRequest`                                     | unverified          | unverified              | no                     | monitor_runtime.rs                          |
| `POST /api/multi-server`                    | body  | `studio_runtime_schema::multi_server_ingress::ManagementRequest`      | yes                 | bounded / review nested | no                     | multi_server.rs → multi_server_ingress.rs   |
| `POST /api/multi-server/v1/auto-pair`       | body  | `studio_runtime_schema::multi_server_ingress::AutoPairRequest`        | yes                 | bounded / review nested | no                     | multi_server.rs → multi_server_ingress.rs   |
| `GET /api/multi-server/v1/identity`         | query | `none / manual`                                                       | unverified          | unverified              | no                     | multi_server.rs                             |
| `POST /api/multi-server/v1/name`            | body  | `studio_runtime_schema::multi_server_ingress::RenameServer`           | yes                 | bounded / review nested | no                     | multi_server.rs → multi_server_ingress.rs   |
| `POST /api/multi-server/v1/pair`            | body  | `studio_runtime_schema::multi_server_ingress::DevicePairRequest`      | yes                 | bounded / review nested | no                     | multi_server.rs → multi_server_ingress.rs   |
| `POST /api/native-command`                  | body  | `NativeCommandRequest`                                                | yes                 | review                  | no                     | catalog_agent.rs                            |
| `POST /api/orchestration/settings`          | body  | `SettingsInput`                                                       | yes                 | bounded / review nested | yes                    | orchestration_settings.rs                   |
| `POST /api/organization`                    | body  | `studio_runtime_services::organization_effects::OrganizationRequest`  | unverified          | unverified              | yes                    | work_items.rs                               |
| `POST /api/peer-teams`                      | body  | `PeerTeamRequest`                                                     | no                  | bounded / review nested | no                     | projects.rs → peer_team_ingress.rs          |
| `GET /api/processes`                        | query | `ProcessListQuery`                                                    | yes                 | bounded / review nested | yes                    | process_controls.rs                         |
| `GET /api/processes/action`                 | query | `ReceiptQuery`                                                        | unverified          | unverified              | yes                    | process_controls.rs                         |
| `POST /api/processes/action`                | body  | `ProcessControlRequest`                                               | yes                 | review                  | yes                    | process_controls.rs → process_controls.rs   |
| `POST /api/profiles`                        | body  | `crate::generated::ProfileBody`                                       | unverified          | unverified              | no                     | panels.rs                                   |
| `GET /api/project-locations`                | query | `LocationQuery`                                                       | yes                 | bounded / review nested | yes                    | projects.rs                                 |
| `POST /api/project-secrets/list`            | body  | `crate::generated::ProjectSecretsListRequest`                         | unverified          | unverified              | no                     | host_secrets.rs                             |
| `POST /api/project-secrets/remove`          | body  | `crate::generated::ProjectSecretRemoveRequest`                        | unverified          | unverified              | no                     | host_secrets.rs                             |
| `POST /api/project-secrets/set`             | body  | `crate::generated::ProjectSecretSetRequest`                           | unverified          | unverified              | no                     | host_secrets.rs                             |
| `POST /api/projects`                        | body  | `ProjectRequest`                                                      | no                  | bounded / review nested | no                     | projects.rs → project_ingress.rs            |
| `POST /api/projects/delete`                 | body  | `crate::generated::ProjectDeleteRequest`                              | unverified          | unverified              | no                     | projects.rs                                 |
| `POST /api/questions/defer`                 | body  | `crate::generated::QuestionDeferBody`                                 | unverified          | unverified              | no                     | questions_write.rs                          |
| `POST /api/questions/delete`                | body  | `crate::generated::QuestionBody`                                      | unverified          | unverified              | no                     | questions_write.rs                          |
| `GET /api/queue`                            | query | `WorkQuery`                                                           | yes                 | review                  | yes                    | work_items.rs                               |
| `POST /api/queue`                           | body  | `unresolved`                                                          | unverified          | unverified              | yes                    | work_items.rs                               |
| `POST /api/rename`                          | body  | `generated::RenameRequest`                                            | unverified          | unverified              | no                     | agents_create.rs                            |
| `GET /api/retained-worktrees`               | query | `RetainedQuery`                                                       | yes                 | bounded / review nested | yes                    | retained_worktrees.rs                       |
| `POST /api/room/delete`                     | body  | `crate::generated::RoomDeleteBody`                                    | unverified          | unverified              | no                     | messages.rs                                 |
| `GET /api/rules`                            | query | `none / manual`                                                       | unverified          | unverified              | no                     | rules.rs                                    |
| `POST /api/rules`                           | body  | `RuleWire`                                                            | no                  | review                  | no                     | rules.rs                                    |
| `GET /api/search`                           | query | `SearchQuery`                                                         | yes                 | bounded / review nested | yes                    | transcript.rs                               |
| `GET /api/search/item`                      | query | `TranscriptItemQuery`                                                 | yes                 | bounded / review nested | yes                    | transcript.rs                               |
| `POST /api/servers/orchestration`           | body  | `Value`                                                               | unverified          | unverified              | no                     | multi_server.rs                             |
| `GET /api/session-cost`                     | query | `SessionCostQuery`                                                    | yes                 | review                  | yes                    | insights.rs                                 |
| `GET /api/skills`                           | query | `SkillsQuery`                                                         | yes                 | review                  | yes                    | catalog_agent.rs                            |
| `POST /api/studio-errors/dismiss`           | body  | `generated::StudioErrorsDismissRequest`                               | unverified          | unverified              | no                     | studio_errors.rs                            |
| `POST /api/sync/drafts`                     | body  | `DraftPushRequest`                                                    | yes                 | bounded / review nested | yes                    | sync_documents.rs                           |
| `POST /api/sync/notify`                     | body  | `studio_runtime_schema::resource_notification::ResourceNotifyRequest` | yes                 | bounded / review nested | no                     | catalog_agent.rs → resource_notification.rs |
| `POST /api/sync/preferences`                | body  | `PreferencePushRequest`                                               | no                  | review                  | no                     | sync_documents.rs                           |
| `GET /api/sync/pull`                        | query | `SyncPullQuery`                                                       | yes                 | review                  | yes                    | sync_documents.rs                           |
| `GET /api/sync/stream`                      | query | `SyncStreamQuery`                                                     | yes                 | bounded / review nested | yes                    | sync_stream.rs                              |
| `POST /api/sync/subscriptions`              | body  | `crate::generated::SyncSubscriptionRequest`                           | unverified          | unverified              | no                     | sync_stream.rs                              |
| `GET /api/task`                             | query | `TaskQuery`                                                           | yes                 | review                  | yes                    | workspace_views.rs                          |
| `GET /api/task/output`                      | query | `TaskOutputQuery`                                                     | yes                 | review                  | yes                    | workspace_views.rs                          |
| `POST /api/terminals/close`                 | body  | `TerminalClose`                                                       | yes                 | bounded / review nested | no                     | terminals.rs → terminal_ingress.rs          |
| `POST /api/terminals/create`                | body  | `TerminalCreateBody`                                                  | yes                 | bounded / review nested | no                     | terminals.rs → terminal_ingress.rs          |
| `POST /api/terminals/input`                 | body  | `TerminalInput`                                                       | yes                 | bounded / review nested | no                     | terminals.rs → terminal_ingress.rs          |
| `GET /api/terminals/output`                 | query | `TerminalOutputQuery`                                                 | yes                 | review                  | yes                    | terminals.rs                                |
| `POST /api/terminals/rename`                | body  | `TerminalRename`                                                      | yes                 | bounded / review nested | no                     | terminals.rs → terminal_ingress.rs          |
| `POST /api/terminals/resize`                | body  | `TerminalResize`                                                      | yes                 | bounded / review nested | no                     | terminals.rs → terminal_ingress.rs          |
| `GET /api/tool-requests`                    | query | `ToolQuery`                                                           | yes                 | review                  | yes                    | tool_requests.rs                            |
| `GET /api/transcript`                       | query | `TranscriptPageQuery`                                                 | yes                 | review                  | yes                    | transcript.rs                               |
| `GET /api/transcript/blob`                  | query | `TranscriptBlobQuery`                                                 | yes                 | bounded / review nested | yes                    | transcript.rs                               |
| `GET /api/transcript/item`                  | query | `TranscriptItemQuery`                                                 | yes                 | bounded / review nested | yes                    | transcript.rs                               |
| `GET /api/transcript/page`                  | query | `TranscriptPageQuery`                                                 | yes                 | review                  | yes                    | transcript.rs                               |
| `GET /api/transcript/search`                | query | `TranscriptSearchQuery`                                               | yes                 | bounded / review nested | yes                    | transcript.rs                               |
| `POST /api/usage-resume`                    | body  | `crate::generated::UsageResumeRequest`                                | unverified          | unverified              | no                     | usage.rs                                    |
| `POST /api/vault/aws-login`                 | body  | `crate::generated::VaultLoginRequest`                                 | unverified          | unverified              | no                     | host_secrets.rs                             |
| `POST /api/vault/pull`                      | body  | `crate::generated::VaultPullRequest`                                  | unverified          | unverified              | no                     | host_secrets.rs                             |
| `POST /api/vault/setup`                     | body  | `crate::generated::VaultSetupRequest`                                 | unverified          | unverified              | no                     | host_secrets.rs                             |
| `POST /api/vault/setup/facts`               | body  | `crate::generated::VaultSetupFactsRequest`                            | unverified          | unverified              | no                     | host_secrets.rs                             |
| `POST /api/vault/unlock`                    | body  | `crate::generated::VaultUnlockRequest`                                | unverified          | unverified              | no                     | host_secrets.rs                             |
| `POST /api/voice/end`                       | body  | `crate::generated::VoiceEndRequest`                                   | unverified          | unverified              | no                     | voice.rs                                    |
| `POST /api/voice/records`                   | body  | `crate::generated::VoiceRecordsRequest`                               | unverified          | unverified              | no                     | voice.rs                                    |
| `POST /api/voice/start`                     | body  | `crate::generated::VoiceStartRequest`                                 | unverified          | unverified              | no                     | voice.rs                                    |
| `POST /api/voice/status`                    | body  | `crate::generated::VoiceStatusRequest`                                | unverified          | unverified              | no                     | voice.rs                                    |
| `POST /api/web-push/remove`                 | body  | `generated::WebPushRemoveRequest`                                     | unverified          | unverified              | no                     | web_push.rs                                 |
| `POST /api/web-push/subscriptions`          | body  | `generated::WebPushSubscribeRequest`                                  | unverified          | unverified              | no                     | web_push.rs                                 |
| `GET /api/work`                             | query | `WorkQuery`                                                           | yes                 | review                  | yes                    | work_items.rs                               |
| `POST /api/work`                            | body  | `WorkRequest`                                                         | no                  | bounded / review nested | yes                    | work_write.rs                               |
| `GET /api/workspace`                        | query | `WorkspaceQuery`                                                      | yes                 | review                  | yes                    | workspace_views.rs                          |
| `GET /api/workspace/tasks`                  | query | `WorkspaceTasksQuery`                                                 | yes                 | review                  | yes                    | workspace_views.rs                          |

## Files needing manual strictness or bounds review

- `account_login.rs`: 13 route parts
- `account_transfer.rs`: 2 route parts
- `agent_management.rs`: 2 route parts
- `agents_create.rs`: 7 route parts
- `branches.rs`: 1 route parts
- `browser.rs`: 1 route parts
- `catalog.rs`: 1 route parts
- `catalog_agent.rs`: 3 route parts
- `checkpoints.rs`: 3 route parts
- `claude_controls.rs`: 3 route parts
- `claude_login.rs`: 5 route parts
- `commands.rs`: 1 route parts
- `desktop.rs`: 1 route parts
- `federation.rs`: 5 route parts
- `host_secrets.rs`: 8 route parts
- `insights.rs`: 2 route parts
- `linux_vm_settings.rs`: 1 route parts
- `mac_sync.rs`: 2 route parts
- `mcp_reconnect.rs`: 1 route parts
- `messages.rs`: 2 route parts
- `models.rs`: 1 route parts
- `monitor_runtime.rs`: 2 route parts
- `monitors.rs`: 1 route parts
- `multi_server.rs`: 2 route parts
- `panels.rs`: 1 route parts
- `process_controls.rs`: 2 route parts
- `projects.rs`: 3 route parts
- `questions_reads.rs`: 2 route parts
- `questions_write.rs`: 4 route parts
- `recovery.rs`: 1 route parts
- `rules.rs`: 2 route parts
- `studio_errors.rs`: 1 route parts
- `sync_documents.rs`: 2 route parts
- `sync_stream.rs`: 1 route parts
- `system_reads.rs`: 1 route parts
- `terminals.rs`: 1 route parts
- `tool_requests.rs`: 1 route parts
- `transcript.rs`: 2 route parts
- `usage.rs`: 5 route parts
- `voice.rs`: 4 route parts
- `web_push.rs`: 2 route parts
- `work_items.rs`: 5 route parts
- `work_write.rs`: 1 route parts
- `workspace_files.rs`: 3 route parts
- `workspace_views.rs`: 5 route parts

## Suggested review split

Counts reflect conservative audit flags, not confirmed defects. Keep each file with one worker; inspect all flagged parts in that file. Generated and external owners need manual confirmation.

- Group 1 (52 flagged parts): `account_login.rs`, `branches.rs`, `browser.rs`, `catalog_agent.rs`, `claude_controls.rs`, `federation.rs`, `insights.rs`, `linux_vm_settings.rs`, `messages.rs`, `models.rs`, `panels.rs`, `process_controls.rs`, `questions_write.rs`, `studio_errors.rs`, `terminals.rs`, `transcript.rs`, `work_items.rs`, `work_write.rs`, `workspace_files.rs`.
- Group 2 (51 flagged parts): `account_transfer.rs`, `agent_management.rs`, `catalog.rs`, `checkpoints.rs`, `claude_login.rs`, `host_secrets.rs`, `mac_sync.rs`, `mcp_reconnect.rs`, `monitor_runtime.rs`, `monitors.rs`, `projects.rs`, `questions_reads.rs`, `recovery.rs`, `system_reads.rs`, `tool_requests.rs`, `usage.rs`, `voice.rs`, `web_push.rs`, `workspace_views.rs`.
- Separate ownership: `desktop.rs`, `rules.rs`, `multi_server.rs`, `sync_documents.rs`, `sync_stream.rs` (vc-http; coordinate incoming signed dispatch); `agents_create.rs` (vi-agent-record); `commands.rs` (vg-ingress-projects).
