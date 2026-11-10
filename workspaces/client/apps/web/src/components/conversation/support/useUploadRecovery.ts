import { useEffect, useMemo } from "react";
import {
  makeAutoObservable,
  observableRef,
  flowResult,
  type CancellablePromise,
} from "mobx";
import { errorText } from "../../../api.ts";
import { onResume } from "../../../sync/resume.ts";
import { retryableReadError } from "../../../sync/readRetry.ts";
import { onUploadsChanged, type PendingUpload } from "../../../sync/uploads.ts";
import { useServerUploads } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { useAttachmentDrafts } from "./useAttachmentDrafts.ts";

type Uploads = ReturnType<typeof useServerUploads>;
type Attach = ReturnType<typeof useAttachmentDrafts>[1];
const UPLOAD_RETRY_BASE_DELAY_MS = 1_000;
const UPLOAD_RETRY_MAX_DELAY_MS = 30_000;

export function createUploadRecoveryRetryScheduler(
  run: (ids: Set<string> | null) => void,
) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let attempt = 0;
  const reset = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    attempt = 0;
  };
  const schedule = (ids: readonly string[] | null) => {
    if (ids && ids.length === 0) {
      reset();
      return;
    }
    if (timer !== undefined) clearTimeout(timer);
    const delay = Math.min(
      UPLOAD_RETRY_BASE_DELAY_MS * 2 ** attempt,
      UPLOAD_RETRY_MAX_DELAY_MS,
    );
    attempt += 1;
    const selectedIds = ids ? new Set(ids) : null;
    timer = setTimeout(() => {
      timer = undefined;
      run(selectedIds);
    }, delay);
  };
  return { reset, schedule, stop: reset };
}

export { retryableReadError as retryableUploadFailure };

export function retryableUploadStoreFailure(error: unknown) {
  return (
    retryableReadError(error) ||
    (typeof DOMException !== "undefined" &&
      error instanceof DOMException &&
      [
        "AbortError",
        "InvalidStateError",
        "TimeoutError",
        "UnknownError",
      ].includes(error.name))
  );
}

export function subscribeUploadRecoveryWakeups(
  refresh: () => void,
  subscribe = onUploadsChanged,
) {
  const stopUploadsChanged = subscribe(refresh);
  const stopResume = onResume(refresh);
  return () => [stopResume, stopUploadsChanged].forEach((stop) => stop());
}

export class UploadRecovery {
  pending: PendingUpload[] = [];
  failures = new Map<string, { text: string; retryable: boolean }>();
  storageError = "";
  busy = false;
  active = false;
  again: boolean | null = null;
  task: CancellablePromise<void> | undefined;
  uploads: Uploads;
  stateDir: string;
  workspaceId: string;
  attach: Attach;
  scheduler: ReturnType<typeof createUploadRecoveryRetryScheduler>;
  constructor(
    uploads: Uploads,
    stateDir: string,
    workspaceId: string,
    attach: Attach,
  ) {
    this.uploads = uploads;
    this.stateDir = stateDir;
    this.workspaceId = workspaceId;
    this.attach = attach;
    this.scheduler = createUploadRecoveryRetryScheduler((ids) =>
      this.wake(ids),
    );
    makeAutoObservable(
      this,
      {
        uploads: false,
        attach: false,
        scheduler: false,
        task: false,
        pending: observableRef,
      },
      { autoBind: true },
    );
  }
  get error() {
    return [
      ...[...this.failures.values()].map((entry) => entry.text),
      this.storageError,
    ]
      .filter(Boolean)
      .join(" ");
  }
  include(rows: PendingUpload[]) {
    this.pending = [
      ...new Map(
        [...this.pending, ...rows].map((row) => [row.id, row]),
      ).values(),
    ];
  }
  get remove() {
    return this.uploads.removeUpload;
  }
  update(rows: PendingUpload[]) {
    const ids = new Set(rows.map((row) => row.id));
    for (const id of this.failures.keys())
      if (!ids.has(id)) this.failures.delete(id);
    this.pending = rows;
    this.storageError = "";
  }
  fail(row: PendingUpload, error: unknown) {
    this.failures.set(row.id, {
      text: `${row.name}: ${errorText(error)}`,
      retryable: retryableUploadStoreFailure(error),
    });
  }
  refresh() {
    this.scheduler.reset();
    this.wake(null);
  }
  retry() {
    this.scheduler.reset();
    this.wake(null, true);
  }
  wake(ids: Set<string> | null = null, retryPermanent = false) {
    if (!this.active || !this.workspaceId) return;
    if (this.busy) {
      this.again ||= retryPermanent;
      return;
    }
    this.task = flowResult(this.run(ids, retryPermanent));
    // Cancellation is the lifecycle's expected completion, handled by stop.
    void this.task.catch(() => {});
  }
  start() {
    this.active = true;
    const stop = subscribeUploadRecoveryWakeups(
      this.refresh,
      this.uploads.onUploadsChanged,
    );
    this.refresh();
    return () => {
      this.active = false;
      this.scheduler.stop();
      stop();
      this.again = null;
      this.task?.cancel();
    };
  }
  *run(retryOnly: Set<string> | null, retryPermanent: boolean) {
    this.busy = true;
    try {
      const rows: PendingUpload[] = yield this.uploads.pendingUploads(
        this.stateDir,
        this.workspaceId,
      );
      this.update(rows);
      if (!rows.length) {
        this.scheduler.schedule([]);
        return;
      }
      if (!navigator.onLine) {
        this.scheduler.reset();
        return;
      }
      const retryableIds = new Set<string>();
      for (const row of rows) {
        if (
          (retryOnly && !retryOnly.has(row.id)) ||
          (!retryPermanent && this.failures.get(row.id)?.retryable === false)
        )
          continue;
        try {
          const asset: Awaited<ReturnType<Uploads["deliverUpload"]>> =
            yield this.uploads.deliverUpload(row);
          if (!asset || this.uploads.uploadCancelled(row.id)) {
            this.failures.delete(row.id);
            continue;
          }
          if (this.workspaceId !== row.workspace) return;
          const saved: boolean = yield this.attach((current) =>
            this.uploads.uploadCancelled(row.id)
              ? current
              : {
                  ...current,
                  [row.agent]: [
                    ...(current[row.agent] || []).filter(
                      (item) => item.id !== asset.id,
                    ),
                    asset,
                  ],
                },
          );
          if (this.uploads.uploadCancelled(row.id)) return;
          if (!saved)
            throw new Error(
              "The uploaded file is saved. Its attachment could not be saved on this device. Keep this page open.",
            );
          yield this.uploads.finishUpload(row.id);
          this.failures.delete(row.id);
        } catch (error) {
          this.fail(row, error);
          if (retryableUploadStoreFailure(error)) retryableIds.add(row.id);
        }
      }
      this.update(
        yield this.uploads.pendingUploads(this.stateDir, this.workspaceId),
      );
      this.scheduler.schedule([...retryableIds]);
    } catch (error) {
      this.storageError = errorText(error);
      if (navigator.onLine && retryableUploadStoreFailure(error))
        this.scheduler.schedule(null);
      else this.scheduler.reset();
    } finally {
      this.busy = false;
      const retry = this.again;
      this.again = null;
      if (retry !== null && this.active) {
        this.scheduler.reset();
        this.wake(null, retry);
      }
    }
  }
}

export function useUploadRecovery(
  stateDir: string,
  workspaceId: string,
  attach: Attach,
) {
  const uploads = useServerUploads();
  const store = useMemo(
    () => new UploadRecovery(uploads, stateDir, workspaceId, attach),
    [uploads, stateDir, workspaceId],
  );
  useEffect(store.start, [store]);
  return store;
}
