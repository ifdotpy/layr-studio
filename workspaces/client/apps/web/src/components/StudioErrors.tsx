import { observer } from "mobx-react-lite";
import { ERROR_LEVELS, isInfoNotice } from "../app/notices/warnings.ts";
import { useConversationActions } from "./conversation/navigation.ts";
import { isNonBlockingWarning } from "../app/notices/nonBlockingWarning.ts";
import { useState } from "react";
import { STUDIO_ERRORS_DISMISS_MAX_IDS } from "../generated/sharedValues.ts";
import {
  Badge,
  Button,
  Checkbox,
  Group,
  SegmentedControl,
  Stack,
  Text,
} from "@mantine/core";
import { errorText } from "../api";
import { appLogEntries, clearAppLog } from "../appLog";
import type { components } from "../generated/api";

export type StudioErrorItem = NonNullable<
  ReturnType<
    import("../generated/observableResources.ts").StateCollections["studioErrors"]["get"]
  >
>;
type Level = components["schemas"]["StudioErrorLevel"];
export const LEVELS = ERROR_LEVELS;
export const LEVEL_COLORS: Record<Level, string> = {
  critical: "violet",
  error: "red",
  warning: "orange",
  log: "blue",
};
/** Sentence-case level name; labels are never all caps. */
const levelLabel = (level: Level) =>
  level.charAt(0).toUpperCase() + level.slice(1);

export function actionLabel(item: StudioErrorItem) {
  switch (item.action) {
    case "resume":
      return "Resume";
    case "check":
      return "Open";
    case "details":
      return "Details";
    default: {
      const exhaustive: never = item;
      return exhaustive;
    }
  }
}
function ownerLabel(item: StudioErrorItem) {
  return item.action === "details" ? "Server" : item.agentName;
}
function when(at: number) {
  return at > 0 ? new Date(at * 1000).toLocaleString() : "";
}

/** When the row's failures happened: one time, or the count and the range. */
export function period(item: StudioErrorItem) {
  const last = when(item.at);
  if (item.count == null || item.count <= 1) return last;
  const first = when(item.firstAt ?? item.at);
  return `${item.count} times${first && last ? `, ${first} to ${last}` : ""}`;
}

/** The rows with their reason and one safe action each. */
export const StudioErrorList = observer(function StudioErrorList({
  onOpen,
}: {
  onOpen?: () => void;
}) {
  const { areas, items: sourceItems, logs } = useStudioErrors();
  const [reply, setReply] = useState<{
    source: StudioErrorItem[];
    items: StudioErrorItem[];
  }>();
  const items = reply?.source === sourceItems ? reply.items : sourceItems;
  const accept = (value: { items: StudioErrorItem[] }) => {
    if (Array.isArray(value.items))
      setReply({ source: sourceItems, items: value.items });
  };
  const [selected, setSelected] = useState(new Set<string>());
  const [busy, setBusy] = useState<string | null>(null);
  const [filter, setFilter] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const { get, post } = areas.scope.services.api;
  const rows = [
    ...items.map((item) => ({
      type: "server" as const,
      level: item.level,
      at: item.at * 1000,
      item,
    })),
    ...logs.map((item) => ({
      type: "local" as const,
      level: item.level,
      at: item.at,
      item,
    })),
  ].sort(
    (a, b) => LEVELS.indexOf(a.level) - LEVELS.indexOf(b.level) || b.at - a.at,
  );
  const visible = filter ? rows.filter((row) => row.level === filter) : rows;
  const visibleServerIds = visible
    .filter((row) => row.type === "server")
    .map((row) => row.item.id);
  const onSelectAll = (ids: string[]) => {
    if (!busy)
      setSelected(new Set(ids.slice(0, STUDIO_ERRORS_DISMISS_MAX_IDS)));
  };
  const onSelect = (id: string, checked: boolean) => {
    if (busy) return;
    setSelected((previous) => {
      const next = new Set(previous);
      if (!checked) next.delete(id);
      else if (next.size < STUDIO_ERRORS_DISMISS_MAX_IDS) next.add(id);
      return next;
    });
  };
  async function onDelete() {
    if (busy || !selected.size) return;
    const ids = [...selected];
    setBusy("delete");
    setError("");
    setNotice("");
    try {
      const result = await post("/api/studio-errors/dismiss", { ids });
      accept(result);
      setSelected(new Set());
      if (result.unknownIds.length)
        setNotice(
          `${result.unknownIds.length} selected errors were already gone.`,
        );
    } catch (failure) {
      let message = `Could not confirm deletion of ${ids.join(", ")}: ${errorText(failure)}`;
      // A lost response permits readback, never another dismissal write.
      try {
        accept(await get("/api/studio-errors"));
      } catch (failure) {
        message += `. Could not refresh: ${errorText(failure)}`;
      }
      setError(message);
    } finally {
      setBusy(null);
    }
  }
  async function onAction(item: StudioErrorItem) {
    if (item.action === "check") {
      onOpen?.();
      areas.open(item.agentId);
      return;
    }
    if (item.action !== "resume" || busy) return;
    setBusy(item.id);
    setError("");
    try {
      await post("/api/connection-recovery", { id: item.agentId });
      await get("/api/studio-errors");
    } catch (failure) {
      setError(errorText(failure));
    } finally {
      setBusy(null);
    }
  }
  return (
    <Stack gap="sm">
      {error && (
        <Text c="red" size="sm" role="alert">
          {error}
        </Text>
      )}
      {notice && (
        <Text size="sm" role="status">
          {notice}
        </Text>
      )}
      <Group gap="xs">
        <SegmentedControl
          aria-label="Filter by level"
          value={filter ?? "all"}
          onChange={(value) => setFilter(value === "all" ? null : value)}
          data={[
            { value: "all", label: `All (${rows.length})` },
            ...LEVELS.map((level) => ({
              value: level,
              label: (
                <span className="studio-error-level">
                  <span
                    className="studio-error-dot"
                    style={{
                      background: `var(--mantine-color-${LEVEL_COLORS[level]}-filled)`,
                    }}
                  />
                  {`${levelLabel(level)} (${rows.filter((row) => row.level === level).length})`}
                </span>
              ),
            })),
          ]}
        />
        {logs.length > 0 && (
          <Button size="xs" variant="subtle" onClick={clearAppLog}>
            Clear local logs
          </Button>
        )}
      </Group>
      <Group gap="xs">
        <Button
          size="xs"
          variant="default"
          disabled={busy !== null || visibleServerIds.length === 0}
          onClick={() => onSelectAll(visibleServerIds)}
        >
          Select all
        </Button>
        <Button
          size="xs"
          variant="default"
          className="studio-error-delete"
          disabled={busy !== null || selected.size === 0}
          onClick={onDelete}
        >
          Delete selected
        </Button>
        <Text size="xs" c="dimmed" role="status">
          {selected.size} selected
        </Text>
      </Group>
      <Text size="xs" c="dimmed">
        Only selected errors are removed from this list. Chats, commands, and
        history are kept.
      </Text>
      {visible.length === 0 ? (
        <Text size="sm">
          {filter
            ? "No entries at this level."
            : "No agent work ended because of a Studio error."}
        </Text>
      ) : (
        <Stack gap="sm" aria-label="Studio errors">
          {visible.map((row) =>
            row.type === "local" ? (
              <article className="studio-error-row" key={row.item.id}>
                <Badge variant="dot" color={LEVEL_COLORS[row.level]}>
                  {levelLabel(row.level)}
                </Badge>
                <Text size="sm">{row.item.title}</Text>
                <Text size="sm">{row.item.reason}</Text>
                {row.item.detail && (
                  <Text size="xs" c="dimmed">
                    {row.item.detail}
                  </Text>
                )}
                <Text size="xs" c="dimmed">
                  {new Date(row.item.at).toLocaleString()}
                </Text>
              </article>
            ) : (
              <article
                className="studio-error-row"
                key={row.item.id}
                data-kind={row.item.kind}
              >
                <Badge variant="dot" color={LEVEL_COLORS[row.level]}>
                  {levelLabel(row.level)}
                </Badge>
                <Checkbox
                  label={ownerLabel(row.item)}
                  aria-label={`Select error ${row.item.id} for ${ownerLabel(row.item)}`}
                  checked={selected.has(row.item.id)}
                  disabled={
                    busy !== null ||
                    (!selected.has(row.item.id) &&
                      selected.size >= STUDIO_ERRORS_DISMISS_MAX_IDS)
                  }
                  onChange={(event) =>
                    onSelect(row.item.id, event.currentTarget.checked)
                  }
                />
                <Text size="sm">{row.item.reason}</Text>
                {row.item.subject && (
                  <Text size="xs" c="dimmed" lineClamp={2}>
                    {row.item.subject}
                  </Text>
                )}
                {row.item.detail && (
                  <Text size="xs" c="dimmed" lineClamp={2}>
                    {row.item.detail}
                  </Text>
                )}
                <details>
                  <summary>Details</summary>
                  <pre className="studio-error-details">
                    {JSON.stringify(row.item, null, 2)}
                  </pre>
                </details>
                <Group gap="xs" mt="xs">
                  {period(row.item) && (
                    <Text size="xs" c="dimmed">
                      {period(row.item)}
                    </Text>
                  )}
                  {row.item.action !== "details" && (
                    <Button
                      size="xs"
                      variant="light"
                      loading={busy === row.item.id}
                      disabled={busy !== null}
                      onClick={() => onAction(row.item)}
                    >
                      {actionLabel(row.item)}
                    </Button>
                  )}
                </Group>
              </article>
            ),
          )}
        </Stack>
      )}
    </Stack>
  );
});

export function useStudioErrors() {
  const areas = useConversationActions();
  const items = [...(areas.state?.studioErrors.values() ?? [])].sort(
    (left, right) => right.at - left.at || left.id.localeCompare(right.id),
  );
  const logs = appLogEntries();
  const accountNotices =
    areas.state?.workspaces
      .values()
      .next()
      .value?.nativeNotices?.filter(
        (notice) =>
          notice.accountKey ===
          (areas.navigation.selectedAgent?.accountKey || "default"),
      ) || [];
  const chatWarnings =
    areas.selected.warningMessages.filter(isNonBlockingWarning);
  const highestLevel = LEVELS.find(
    (level) =>
      items.some((item) => item.level === level) ||
      logs.some((item) => item.level === level) ||
      (level === "warning" &&
        (accountNotices.some((notice) => !isInfoNotice(notice)) ||
          chatWarnings.length > 0)),
  );
  const hasWarning = highestLevel !== undefined && highestLevel !== "log";
  const noticeCount = accountNotices.length + chatWarnings.length;
  return {
    areas,
    items,
    logs,
    accountNotices,
    chatWarnings,
    highestLevel,
    hasWarning,
    noticeCount,
    warningTitle:
      hasWarning || (noticeCount === 0 && logs.length === 0)
        ? "Warnings"
        : "Notices",
    warningCount: noticeCount + items.length + logs.length,
  };
}
