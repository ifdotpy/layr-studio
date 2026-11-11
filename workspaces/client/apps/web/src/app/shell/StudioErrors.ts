import { makeAutoObservable } from "mobx";
import type { ServerAreas } from "../contracts.ts";
import { ERROR_LEVELS, isInfoNotice } from "../notices/warnings.ts";
import { isNonBlockingWarning } from "../notices/nonBlockingWarning.ts";
import { STUDIO_ERRORS_DISMISS_MAX_IDS } from "../../generated/sharedValues.ts";
import { appLogEntries } from "../../appLog.ts";
import { errorText } from "../../api.ts";
import { flowValue } from "../../flowValue.ts";
import type { StudioErrorItem } from "../../components/shell/StudioErrorList/StudioErrorList.tsx";

export class StudioErrors {
  readonly areas;
  selected = new Set<string>();
  busy: string | null = null;
  filter: string | null = null;
  error = "";
  notice = "";
  constructor(areas: ServerAreas) {
    this.areas = areas;
    makeAutoObservable(this, { areas: false }, { autoBind: true });
  }
  get rows() {
    const { items, logs } = this.summary;
    return [
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
      (a, b) =>
        ERROR_LEVELS.indexOf(a.level) - ERROR_LEVELS.indexOf(b.level) ||
        b.at - a.at,
    );
  }
  get visible() {
    return this.filter
      ? this.rows.filter((row) => row.level === this.filter)
      : this.rows;
  }
  get visibleServerIds() {
    return this.visible
      .filter((row) => row.type === "server")
      .map((row) => row.item.id);
  }
  setFilter(value: string) {
    this.filter = value === "all" ? null : value;
  }
  onSelectAll(ids: string[]) {
    if (!this.busy)
      this.selected = new Set(ids.slice(0, STUDIO_ERRORS_DISMISS_MAX_IDS));
  }
  onSelect(id: string, checked: boolean) {
    if (this.busy) return;
    if (!checked) this.selected.delete(id);
    else if (this.selected.size < STUDIO_ERRORS_DISMISS_MAX_IDS)
      this.selected.add(id);
  }
  *onDelete() {
    if (this.busy || !this.selected.size) return;
    const ids = [...this.selected];
    const { get, post } = this.areas.scope.services.api;
    this.busy = "delete";
    this.error = this.notice = "";
    try {
      const result = yield* flowValue(
        post("/api/studio-errors/dismiss", { ids }),
      );
      this.selected.clear();
      if (result.unknownIds.length)
        this.notice = `${result.unknownIds.length} selected errors were already gone.`;
    } catch (failure) {
      let message = `Could not confirm deletion of ${ids.join(", ")}: ${errorText(failure)}`;
      // Read back after a lost response without repeating the dismissal write.
      try {
        yield* flowValue(get("/api/studio-errors"));
      } catch (failure) {
        message += `. Could not refresh: ${errorText(failure)}`;
      }
      this.error = message;
    } finally {
      this.busy = null;
    }
  }
  *onAction(item: StudioErrorItem, onOpen?: () => void) {
    if (item.action === "check") {
      onOpen?.();
      this.areas.open(item.agentId);
      return;
    }
    if (item.action !== "resume" || this.busy) return;
    this.busy = item.id;
    this.error = "";
    const { get, post } = this.areas.scope.services.api;
    try {
      yield* flowValue(post("/api/connection-recovery", { id: item.agentId }));
      yield* flowValue(get("/api/studio-errors"));
    } catch (failure) {
      this.error = errorText(failure);
    } finally {
      this.busy = null;
    }
  }
  get summary() {
    const areas = this.areas;
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
    const highestLevel = ERROR_LEVELS.find(
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
}
