import { observer } from "mobx-react-lite";
import { ERROR_LEVELS } from "../../../app/notices/warnings.ts";
import { useConversationActions } from "../../conversation/support/navigation.ts";
import { STUDIO_ERRORS_DISMISS_MAX_IDS } from "../../../generated/sharedValues.ts";
import {
  Badge,
  Button,
  Checkbox,
  Group,
  SegmentedControl,
  Stack,
  Text,
} from "@mantine/core";
import { clearAppLog } from "../../../appLog.ts";
import type { components } from "../../../generated/api.ts";

export type StudioErrorItem = NonNullable<
  ReturnType<
    import("../../../generated/observableResources.ts").StateCollections["studioErrors"]["get"]
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
  const model = useConversationActions().ui.errors;
  const { logs } = model.summary;
  return (
    <Stack gap="sm">
      {model.error && (
        <Text c="red" size="sm" role="alert">
          {model.error}
        </Text>
      )}
      {model.notice && (
        <Text size="sm" role="status">
          {model.notice}
        </Text>
      )}
      <Group gap="xs">
        <SegmentedControl
          aria-label="Filter by level"
          value={model.filter ?? "all"}
          onChange={model.setFilter}
          data={[
            { value: "all", label: `All (${model.rows.length})` },
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
                  {`${levelLabel(level)} (${model.rows.filter((row) => row.level === level).length})`}
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
          disabled={model.busy !== null || model.visibleServerIds.length === 0}
          onClick={() => model.onSelectAll(model.visibleServerIds)}
        >
          Select all
        </Button>
        <Button
          size="xs"
          variant="default"
          className="studio-error-delete"
          disabled={model.busy !== null || model.selected.size === 0}
          onClick={model.onDelete}
        >
          Delete selected
        </Button>
        <Text size="xs" c="dimmed" role="status">
          {model.selected.size} selected
        </Text>
      </Group>
      <Text size="xs" c="dimmed">
        Only selected errors are removed from this list. Chats, commands, and
        history are kept.
      </Text>
      {model.visible.length === 0 ? (
        <Text size="sm">
          {model.filter
            ? "No entries at this level."
            : "No agent work ended because of a Studio error."}
        </Text>
      ) : (
        <Stack gap="sm" aria-label="Studio errors">
          {model.visible.map((row) => (
            <article
              className="studio-error-row"
              key={row.item.id}
              data-kind={row.type === "server" ? row.item.kind : undefined}
            >
              <Badge variant="dot" color={LEVEL_COLORS[row.level]}>
                {levelLabel(row.level)}
              </Badge>
              {row.type === "local" ? (
                <Text size="sm">{row.item.title}</Text>
              ) : (
                <Checkbox
                  label={ownerLabel(row.item)}
                  aria-label={`Select error ${row.item.id} for ${ownerLabel(row.item)}`}
                  checked={model.selected.has(row.item.id)}
                  disabled={
                    model.busy !== null ||
                    (!model.selected.has(row.item.id) &&
                      model.selected.size >= STUDIO_ERRORS_DISMISS_MAX_IDS)
                  }
                  onChange={(event) =>
                    model.onSelect(row.item.id, event.currentTarget.checked)
                  }
                />
              )}
              <Text size="sm">{row.item.reason}</Text>
              {row.type === "server" && row.item.subject && (
                <Text size="xs" c="dimmed" lineClamp={2}>
                  {row.item.subject}
                </Text>
              )}
              {row.item.detail && (
                <Text
                  size="xs"
                  c="dimmed"
                  {...(row.type === "server" ? { lineClamp: 2 } : {})}
                >
                  {row.item.detail}
                </Text>
              )}
              {row.type === "local" ? (
                <Text size="xs" c="dimmed">
                  {new Date(row.item.at).toLocaleString()}
                </Text>
              ) : (
                <>
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
                        loading={model.busy === row.item.id}
                        disabled={model.busy !== null}
                        onClick={() => model.onAction(row.item, onOpen)}
                      >
                        {actionLabel(row.item)}
                      </Button>
                    )}
                  </Group>
                </>
              )}
            </article>
          ))}
        </Stack>
      )}
    </Stack>
  );
});

export function useStudioErrors() {
  return useConversationActions().ui.errors.summary;
}
