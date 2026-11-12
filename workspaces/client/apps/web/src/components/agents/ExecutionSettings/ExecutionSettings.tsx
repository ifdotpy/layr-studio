import { ExecutionSummaryTrigger } from "../ExecutionSummaryTrigger/ExecutionSummaryTrigger.tsx";
import {
  Button,
  Modal,
  NativeSelect,
  Popover,
  SegmentedControl,
  Stack,
  Switch,
  Text,
} from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { observer } from "mobx-react-lite";
import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
  type RefObject,
} from "react";
import { type PostBody } from "../../../api.ts";
import { AgentPermissionMode, AgentStatus } from "../../../generated/api.ts";
import { agentId } from "../../../generated/observableResources.ts";
import {
  ProviderKind,
  providerSupports,
} from "../../../generated/providerKind.ts";
import { resourceData } from "../../../store/runtime.ts";
import { layerZIndex } from "../../../theme.ts";
import { busy, type Agent } from "../../../types.ts";
import { setupAccountName } from "../../accounts/AccountTiles/AccountTiles.tsx";
import { AccountTransferStatus } from "../../accounts/AccountTransferStatus/AccountTransferStatus.tsx";
import type { Account } from "../../accounts/support/Accounts.tsx";
import {
  useServerApi,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import BottomSheet from "../../ui/BottomSheet/BottomSheet.tsx";
import {
  AgentSetupPicker,
  type SetupRole,
} from "../AgentSetupPicker/AgentSetupPicker.tsx";
import { subagentsOff } from "../SubagentConcurrencyControl/SubagentConcurrencyControl.tsx";
import { type AgentsUi } from "../support/AgentsUi.ts";
import {
  useModelCatalogReader,
  useWorkerModels,
} from "../support/WorkerModelPicker.tsx";
import type {
  AgentSettingsInput,
  ExecutionSettingsInput,
} from "../support/ExecutionSettingsStore.ts";
import {
  accountOf,
  detailOf,
  executionScope,
  reviewSettingsFor,
  settingsFor,
  settingsLabel,
  summaryOf,
} from "../support/ExecutionSettingsStore.ts";
import {
  executionPicker,
  projectExecution,
} from "../support/executionProjection.ts";
export {
  settingsFor,
  type AgentSettingsInput,
  type ExecutionSettingsInput,
} from "../support/ExecutionSettingsStore.ts";

import { SettingsSection } from "../../ui/PanelHeader/PanelHeader.tsx";
import {
  observeSettingsChange,
  type SettingsChange,
} from "./liveSettingsStatus.ts";

type Catalog = ReturnType<typeof useWorkerModels>;
type ConversationBody = PostBody<"/api/conversation">;
type SettingsProps = {
  agentsUi: AgentsUi;
  presentation?: "menu" | "panel" | undefined;
  renderTrigger?: (props: {
    agent: Agent;
    detail?: Agent | undefined;
    workerDetail?: Agent | undefined;
    reviewDetail?: Agent | undefined;
    opened: boolean;
    ref: RefObject<HTMLButtonElement | null>;
    onClick: () => void;
  }) => ReactElement;
  input: ExecutionSettingsInput;
  leadInput?: AgentSettingsInput | undefined;
  catalog: Catalog;
  accounts?: readonly Account[];
  team?: Array<Pick<Agent, "id" | "rootId" | "deletedAt" | "accountKey">>;
  initialRole?: SetupRole;
  settingsRow?: boolean;
  onAccountChange?: (key: string) => void;
  workerExcludedAccountKeys?: readonly string[] | undefined;
  reviewerExcludedAccountKeys?: readonly string[] | undefined;
  onWorkerExcludedAccountKeysChange?: ((keys: string[]) => void) | undefined;
  onReviewerExcludedAccountKeysChange?: ((keys: string[]) => void) | undefined;
  accountDisabled?: boolean;
  accountModalOpen?: boolean;
  onDraftChange?: (patch: Partial<ConversationBody>) => void;
};
export { ExecutionSettings };

// These fields belong to the Team section, outside the complete model picker.
export function DelegationSettings(props: SettingsProps) {
  return (
    <ExecutionSettings {...props} initialRole="worker" section="delegation" />
  );
}
export function ExecutionPermissions(props: SettingsProps) {
  return <ExecutionSettings {...props} section="permissions" />;
}
type ControllerProps = SettingsProps & {
  section?: "delegation" | "permissions";
};
const ExecutionSettings = observer(function ExecutionSettings(
  props: ControllerProps,
) {
  const subject = summaryOf(props.input);
  const detail = detailOf(props.input);
  const ui = props.agentsUi;
  const store = useServerStore();
  const settingsScope = JSON.stringify([subject.id, props.section || "model"]);
  const [storedChange, setChangeStatus] = ui.cell(
    settingsScope,
    "changeStatus",
  );
  const changeStatus = storedChange
    ? observeSettingsChange(storedChange, subject, detail || {})
    : null;
  const [statusOpen, setStatusOpen] = useState(false);
  const statusModal = changeStatus && (
    <Modal
      opened={statusOpen}
      onClose={() => setStatusOpen(false)}
      title="Settings change"
      centered
      zIndex={layerZIndex.modalAbovePopover}
    >
      <Stack gap="sm">
        <Text role="status">
          {changeStatus.phase === "saving"
            ? "Saving…"
            : changeStatus.phase === "queued"
              ? "Queued for the next turn"
              : changeStatus.phase === "saved"
                ? "Saved"
                : "Failed"}
        </Text>
        <Text size="sm">
          {changeStatus.before.model || "Default"} →{" "}
          {changeStatus.requested.model || "Default"} ·{" "}
          {changeStatus.before.effort || "Default"} →{" "}
          {changeStatus.requested.effort || "Default"} · Fast{" "}
          {changeStatus.before.fastMode ? "on" : "off"} →{" "}
          {changeStatus.requested.fastMode ? "on" : "off"}
        </Text>
        {changeStatus.phase === "queued" && (
          <Text size="sm">
            {subject.status === AgentStatus.idle ||
            subject.status === AgentStatus.completed
              ? "If still queued, the change applies when this agent next runs. Check current settings for its latest state."
              : "If still queued, the change applies at the next turn. Check current settings for its latest state."}
          </Text>
        )}
        {changeStatus.phase === "saving" && changeStatus.error && (
          <Text size="sm">
            {changeStatus.error} Use Check settings save to confirm the same
            request.
          </Text>
        )}
        {changeStatus.phase === "failed" && (
          <Text size="sm">
            {changeStatus.error} See App errors for details.
          </Text>
        )}
        <Button onClick={() => setStatusOpen(false)}>Close</Button>
      </Stack>
    </Modal>
  );
  const [confirmed] = ui.cell(settingsScope, "confirmed");
  const signature = (role: SetupRole, value: Agent = detail || subject) =>
    JSON.stringify(
      role === "review"
        ? reviewSettingsFor(value)
        : settingsFor(subject, role === "worker", value),
    );
  const readSignature = (role: SetupRole) => {
    const current = resourceData(
      store.agentDetail(agentId(subject.id)).current,
    );
    return JSON.stringify([
      subject.id,
      accountOf(subject),
      signature(role, current || detail || subject),
    ]);
  };
  const beginSettingsSave = (role: SetupRole) =>
    ui.begin(settingsScope, role, () => readSignature(role));
  const receipt = (role: SetupRole) => {
    const saved = confirmed[role];
    return saved?.value.id && saved.baseline === readSignature(role)
      ? saved.value
      : undefined;
  };
  const main = receipt("orchestrator");
  const worker = receipt("worker");
  const review = receipt("review");
  // Only execution fields bridge the response-to-SSE interval. Status and
  // other agent fields continue to come from the workspace projection.
  const [storedRole, setRole] = ui.cell(settingsScope, "role");
  const selectedRole =
    storedRole ||
    props.initialRole ||
    (subject.isLead ? "orchestrator" : "worker");
  // With subagents off, the composer picker of a lead has no Workers role; the
  // settings rows still set the defaults for later subagents.
  const workersHidden =
    !props.settingsRow && !!subject.isLead && subagentsOff(detail || {});
  const role =
    workersHidden && selectedRole === "worker" ? "orchestrator" : selectedRole;
  const [opened, setOpened] = ui.cell(settingsScope, "opened");
  const [transferOpen, setTransferOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const changeOpened = (value: boolean) => {
    setOpened(value);
    if (!value) {
      triggerRef.current?.focus({ preventScroll: true });
    }
    if (!value && props.settingsRow)
      setRole(
        props.initialRole || (subject.isLead ? "orchestrator" : "worker"),
      );
  };
  const inheritedRole = !subject.isLead && role !== "worker";
  const input = inheritedRole ? props.leadInput || props.input : props.input;
  const agent = summaryOf(input);
  const readOnlyReason = inheritedRole
    ? props.leadInput?.summary
      ? "Inherited from the lead. Open the lead chat to change these settings."
      : "Managed by the lead. Its settings are unavailable in this chat."
    : undefined;
  const roleSwitchId = useId();
  const focusRole = useRef(false);
  useLayoutEffect(() => {
    const selected = document
      .getElementById(roleSwitchId)
      ?.querySelector<HTMLInputElement>("input:checked");
    // Each role has isolated draft state. Preserve the selected keyboard
    // target when its form remounts inside the stable focus trap.
    selected?.setAttribute("data-autofocus", "true");
    if (focusRole.current) {
      selected?.focus();
      focusRole.current = false;
    }
  }, [role, roleSwitchId, opened]);
  const updateChange = (key: string, patch: Partial<SettingsChange>) =>
    setChangeStatus((current) =>
      current &&
      current.phase !== "failed" &&
      current.requestKey === key &&
      subject.id === current.agentId &&
      accountOf(subject) === current.accountKey
        ? { ...current, ...patch }
        : current,
    );
  const scope = JSON.stringify([agent.id, accountOf(agent), role]);
  const setErrorForOpen = () => {
    const form = executionScope(agent, role, props.section);
    ui.set(form, "error", "");
    ui.set(form, "status", null);
  };
  const controls = (
    <ScopedExecutionSettings
      key={scope}
      {...props}
      beginSettingsSave={beginSettingsSave}
      acknowledged={receipt(role)}
      transferOpen={transferOpen}
      setTransferOpen={setTransferOpen}
      onSettingsChange={(change) => {
        setChangeStatus(change);
        setStatusOpen(true);
      }}
      onSettingsFailure={(key, error, uncertain) =>
        updateChange(key, {
          phase: uncertain ? "saving" : "failed",
          error,
          errorLogged: true,
        })
      }
      onSettingsConfirmed={(key, nextTurn, canonical) =>
        updateChange(key, {
          phase: nextTurn ? "queued" : "saved",
          requested: {
            model: settingsFor(canonical, false).model,
            effort: settingsFor(canonical, false).effort,
            fastMode: settingsFor(canonical, false).fast_mode,
          },
        })
      }
      presentation={props.renderTrigger ? "panel" : props.presentation}
      input={input}
      inheritedUnavailable={inheritedRole && !props.leadInput?.summary}
      readOnlyReason={readOnlyReason}
      roleSwitchId={roleSwitchId}
      teamDefaults={role === "review" || (role === "worker" && !!agent.isLead)}
      role={role}
      workersHidden={workersHidden}
      onRole={(value) => {
        focusRole.current = true;
        setRole(value);
      }}
      opened={opened}
      triggerRef={triggerRef}
      setOpened={changeOpened}
    />
  );
  if (props.section) return controls;
  if (props.presentation === "panel")
    return (
      <>
        {controls}
        {statusModal}
      </>
    );
  return (
    <ExecutionMenu
      opened={opened}
      setOpened={changeOpened}
      accountModalOpen={props.accountModalOpen || transferOpen || statusOpen}
      statusModal={statusModal}
      label={settingsLabel(
        agent,
        role,
        role === "review" || (role === "worker" && !!agent.isLead),
      )}
      trigger={
        props.renderTrigger ? (
          props.renderTrigger({
            agent: subject,
            detail: main || detail,
            workerDetail: worker,
            reviewDetail: review,
            opened,
            ref: triggerRef,
            onClick: () => changeOpened(!opened),
          })
        ) : (
          <ExecutionSummaryTrigger
            agent={subject}
            detail={main || detail}
            workerDetail={worker}
            reviewDetail={review}
            team={props.team || []}
            lead={props.leadInput?.summary}
            ref={triggerRef}
            onOpen={() => {
              setErrorForOpen();
              changeOpened(!opened);
            }}
          />
        )
      }
    >
      {controls}
    </ExecutionMenu>
  );
});

export type ScopedSettingsProps = ControllerProps & {
  acknowledged: Agent | undefined;
  transferOpen: boolean;
  setTransferOpen: (value: boolean) => void;
  beginSettingsSave: (role: SetupRole) => (agent: Agent) => boolean;
  onSettingsChange: (change: SettingsChange) => void;
  onSettingsFailure: (key: string, message: string, uncertain: boolean) => void;
  onSettingsConfirmed: (
    key: string,
    nextTurn: boolean,
    canonical: Agent,
  ) => void;
  teamDefaults: boolean;
  readOnlyReason?: string | undefined;
  inheritedUnavailable: boolean;
  roleSwitchId: string;
  role: SetupRole;
  workersHidden?: boolean;
  onRole: (role: SetupRole) => void;
  opened: boolean;
  triggerRef: RefObject<HTMLButtonElement | null>;
  setOpened: (opened: boolean) => void;
};
const ScopedExecutionSettings = observer(function ScopedExecutionSettings(
  argumentsForProjection: ScopedSettingsProps,
) {
  const {
    input,
    presentation = "menu",
    renderTrigger,
    section,
    readOnlyReason,
    teamDefaults = false,
    settingsRow = false,
    accountModalOpen = false,
    onDraftChange,
    role,
    opened,
    setOpened,
    transferOpen,
    setTransferOpen,
  } = argumentsForProjection;
  const api = useServerApi();
  const store = useServerStore();
  const catalogReader = useModelCatalogReader();
  const sourceAgent = summaryOf(input);
  const accountCatalog = useWorkerModels(
    accountOf(sourceAgent),
    teamDefaults && (opened || settingsRow || presentation === "panel"),
    teamDefaults,
  );
  const inheritedCatalog = useWorkerModels(
    accountOf(sourceAgent),
    !!readOnlyReason && role === "orchestrator",
  );
  const browse = useState<string | null>(null);
  const execution = argumentsForProjection.agentsUi.execution;
  const view = projectExecution(argumentsForProjection, {
    api,
    store,
    catalogReader,
    accountCatalog,
    inheritedCatalog,
    browse,
  });
  useEffect(() => {
    if (!opened) return;
    const close = (event: KeyboardEvent) => {
      const cameFromModal = event
        .composedPath()
        .some(
          (target) =>
            target instanceof Element &&
            target.classList.contains("mantine-Modal-content"),
        );
      if (
        event.key === "Escape" &&
        !cameFromModal &&
        !accountModalOpen &&
        !transferOpen &&
        (event.target as HTMLElement | null)?.getAttribute(
          "data-mantine-stop-propagation",
        ) !== "true"
      ) {
        event.preventDefault();
        event.stopPropagation();
        setOpened(false);
      }
    };
    document.addEventListener("keydown", close, true);
    return () => document.removeEventListener("keydown", close, true);
  }, [accountModalOpen, transferOpen, opened]);
  const delegation =
    teamDefaults && view.agent.isLead && !readOnlyReason && !onDraftChange ? (
      <NativeSelect
        label="Delegation depth"
        className="execution-delegation"
        value={String(view.current.delegation_depth)}
        description="Existing workers keep their depth."
        title="Lowering blocks new delegation immediately. Existing descendants keep running."
        data={[
          { value: "1", label: "Lead only (1)" },
          { value: "2", label: "One nested level (2)" },
          { value: "3", label: "Two nested levels (3)" },
        ]}
        disabled={view.saving}
        onChange={(event) =>
          void execution.change(view, {
            delegation_depth: Number(event.currentTarget.value),
          })
        }
      />
    ) : null;
  const controls = (
    <>
      {role === "review" && !view.reviewCurrent.model && (
        <Text size="sm" c="dimmed">
          Select a review model
        </Text>
      )}
      <AgentSetupPicker {...executionPicker(argumentsForProjection, view)} />{" "}
      {role === "review" && view.reviewCurrent.fast_mode === undefined && (
        <p className="notice">
          Native reviews use this agent’s Fast setting; review tasks use
          Workers.
        </p>
      )}
      {role === "worker" &&
        teamDefaults &&
        !readOnlyReason &&
        !onDraftChange && (
          <p className="notice">
            For new subagents. Existing subagents keep their account and model.
          </p>
        )}
      {!readOnlyReason && (view.blockedTransfer || transferOpen) && (
        <AccountTransferStatus
          opened={transferOpen}
          onClose={() => setTransferOpen(false)}
          transfer={view.shownTransfer}
          targetLabel={
            teamDefaults
              ? setupAccountName(view.shownTransferTarget)
              : view.shownTransferTarget?.email ||
                setupAccountName(view.shownTransferTarget)
          }
          error={view.error}
          pending={view.saving}
          onAction={(action) => void execution.transfer(view, action)}
        />
      )}
      {view.blockedTransfer && !transferOpen && (
        <Button
          size="compact-xs"
          variant="subtle"
          onClick={() => setTransferOpen(true)}
        >
          View transfer status
        </Button>
      )}
      {view.status && (
        <p role="status" className="notice execution-status">
          {view.status.kind === "saving"
            ? "Saving…"
            : `Saved${view.status.text ? ` · ${view.status.text}` : ""}`}
        </p>
      )}
      {((!teamDefaults && view.queued) ||
        (view.active && view.canQueueSettings)) && (
        <p className="notice" role="status">
          Model settings apply to the next turn. The current response keeps its
          settings.
          {view.modeQueued &&
            ` Current mode: ${view.detail?.daybreakEnabled ? "Daybreak" : "Standard"}. Next turn: ${view.modeLabel}.`}
        </p>
      )}
      {view.active &&
        view.queued &&
        providerSupports(view.agent.provider, "current_turn_settings") &&
        view.detail?.turnId && (
          <Button
            size="compact-xs"
            variant="subtle"
            disabled={view.disabled}
            onClick={() => void execution.applyCurrentTurn(view)}
          >
            Apply queued settings to current turn
          </Button>
        )}
      {view.active && !view.canQueueSettings && (
        <p className="notice">
          Available when this turn ends. Update the server to set options for
          the next turn.
        </p>
      )}
      {view.catalog.error && (
        <div role="alert">
          <p>{view.catalog.error}</p>
          <Button onClick={view.catalog.retry}>Retry model list</Button>
        </div>
      )}
      {!readOnlyReason && view.unconfirmed && !view.saving && (
        <div role="status">
          <p>
            The settings response is unconfirmed. Check the same save before
            changing settings again.
          </p>
          <Button
            disabled={view.saving}
            loading={view.saving}
            onClick={() =>
              view.unconfirmed && void execution.submit(view, view.unconfirmed)
            }
          >
            Check settings save
          </Button>
        </div>
      )}
      {view.error && (
        <p className="execution-error" role="alert">
          {view.error}
        </p>
      )}
    </>
  );
  const fullAccess =
    view.pendingYolo?.value ??
    view.detail?.permissionMode === AgentPermissionMode["bypassPermissions"];
  const askingNextTurn =
    (!!view.agent.inFlight || busy.has(view.agent.status ?? "")) && !fullAccess;
  const permissions =
    view.agent.isLead &&
    view.agent.provider === ProviderKind.Codex &&
    providerSupports(view.agent.provider, "approvals") ? (
      settingsRow ? (
        <div className="execution-setting">
          <Text size="sm" fw={500}>
            Permissions
          </Text>
          <SegmentedControl
            className="execution-setting-control"
            fullWidth
            aria-label="Full access without approval"
            value={fullAccess ? "full" : "ask"}
            disabled={view.saving || view.detail?.permissionMode === undefined}
            data={[
              { value: "ask", label: "Ask" },
              { value: "full", label: "Full" },
            ]}
            onChange={(value) =>
              void execution.changeYolo(view, value === "full")
            }
          />
          <Text size="xs" c="dimmed" className="execution-setting-description">
            {view.detail?.permissionMode === undefined
              ? "Available after the server update."
              : askingNextTurn
                ? "Ask policy changes apply from the next turn."
                : "Applies immediately to the whole team."}
          </Text>
          {view.error && (
            <p className="execution-error" role="alert">
              {view.error}
            </p>
          )}
        </div>
      ) : (
        <SettingsSection title="Permissions">
          <p>Applies to the whole team.</p>
          <Switch
            label="Full access without approval"
            aria-label="Full access without approval"
            checked={fullAccess}
            disabled={view.saving || view.detail?.permissionMode === undefined}
            description={
              view.detail?.permissionMode === undefined
                ? "Available after the server update."
                : askingNextTurn
                  ? "Ask policy changes apply from the next turn."
                  : view.detail?.permissionMode == null
                    ? "The team uses normal Codex permissions."
                    : fullAccess
                      ? "Tools run without permission prompts."
                      : "Tools ask for permission."
            }
            onChange={(event) =>
              void execution.changeYolo(view, event.currentTarget.checked)
            }
          />
          {view.saving && <p role="status">Saving…</p>}
          {view.error && (
            <p className="execution-error" role="alert">
              {view.error}
            </p>
          )}
        </SettingsSection>
      )
    ) : null;
  if (section === "delegation")
    return (
      <Stack gap="sm">
        {delegation}
        {view.error && <p role="alert">{view.error}</p>}
      </Stack>
    );
  if (section === "permissions") return permissions;
  return (
    <div
      className={renderTrigger ? undefined : "execution-profile-body"}
      data-role={role}
    >
      {controls}
    </div>
  );
});

/** Keep the anchor and focus trap mounted while the isolated role form changes. */
function ExecutionMenu({
  opened,
  setOpened,
  accountModalOpen = false,
  statusModal,
  label,
  trigger,
  children,
}: {
  opened: boolean;
  setOpened: (value: boolean) => void;
  accountModalOpen?: boolean | undefined;
  statusModal?: ReactNode;
  label: string;
  trigger: ReactElement;
  children: ReactNode;
}) {
  const mobile = useMediaQuery("(max-width: 760px)");
  const popoverLabelId = useId();
  if (mobile)
    return (
      <>
        {trigger}
        <BottomSheet
          opened={opened}
          onClose={() => setOpened(false)}
          closeOnEscape={false}
          closeOnClickOutside={!accountModalOpen}
          trapFocus={!accountModalOpen}
          returnFocus
          title={label}
          className="setup-sheet"
        >
          <div className="execution-dropdown" role="region" aria-label={label}>
            {children}
          </div>
        </BottomSheet>
        {statusModal}
      </>
    );
  return (
    <>
      <Popover
        opened={opened}
        onChange={setOpened}
        closeOnClickOutside={!accountModalOpen}
        trapFocus={!accountModalOpen}
        zIndex={layerZIndex.popover}
        position="top-start"
        shadow="md"
        // Mantine's default max-content width would override the picker token.
        width="var(--studio-execution-dropdown-size)"
        middlewares={{ flip: true, shift: true, size: true }}
        closeOnEscape={false}
        returnFocus
      >
        <Popover.Target>{trigger}</Popover.Target>
        <Popover.Dropdown
          className="execution-dropdown"
          role="dialog"
          aria-labelledby={popoverLabelId}
          aria-label={label}
        >
          <span id={popoverLabelId} className="sr-only">
            {label}
          </span>
          {children}
        </Popover.Dropdown>
      </Popover>
      {statusModal}
    </>
  );
}

export {
  observeSettingsChange,
  type SettingsChange,
} from "./liveSettingsStatus.ts";
