import { observable, runInAction } from "mobx";
import { UNSENT_FAILURE_EVENT, draftStore } from "../unsent/store.ts";
import { serverViewId } from "../servers/environment.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import type { DraftVersion } from "../generated/rustOwned.ts";
import type { components } from "../generated/api.ts";
import { createDirectDrafts } from "./directDrafts.ts";
import { decodeDraftVersion } from "./draftDecoder.ts";
import { encodeDraftPayload } from "./draftPayload.ts";
import { draftVersionId } from "./draftIdentity.ts";
import { activeDraftVersions } from "./draftVersions.ts";
import { DRAFT_SYNC_TIMING_MS } from "./draftSyncTiming.ts";

export type { DraftVersion } from "../generated/rustOwned.ts";
type Drafts = Record<string, string>;
const incompleteDrafts = "Some saved drafts could not be loaded.";

export function createDrafts(options: {
  serverId?: string;
  api: {
    syncPost: (
      path: "/api/sync/drafts",
      body: components["schemas"]["DraftPushRequest"],
    ) => Promise<components["schemas"]["SyncDocument"][]>;
  };
  resources: Pick<
    typeof import("./resourceEvents.ts"),
    "watchResourceChanges" | "replaceStream"
  >;
}) {
  const device = randomUUID();
  const writer = randomUUID();
  const metadata = observable({
    conflicts: [] as DraftVersion[],
    error: "",
    unsentError: "",
  });
  const setConflicts = (update: (old: DraftVersion[]) => DraftVersion[]) =>
    runInAction(() => {
      metadata.conflicts = update(metadata.conflicts);
    });
  const setUnsentError = (value: string) =>
    runInAction(() => {
      metadata.unsentError = value;
    });
  let current: Drafts = {};
  const listeners = new Map<string, Set<() => void>>();
  let versions: DraftVersion[] = [];
  const dismissed = new Set<string>();

  const setError = (next: string | ((previous: string) => string)) =>
    runInAction(() => {
      metadata.error = typeof next === "function" ? next(metadata.error) : next;
    });

  let quiet: ReturnType<typeof setTimeout> | undefined;
  let maximum: ReturnType<typeof setTimeout> | undefined;
  const key = (version: DraftVersion) =>
    JSON.stringify([version.id, version.text]);
  const publish = (previous: Drafts, next: Drafts) => {
    for (const [session, callbacks] of listeners)
      if (previous[session] !== next[session])
        callbacks.forEach((callback) => callback());
  };
  const reconcile = () => {
    const records = store
      .records()
      .map((row) => decodeDraftVersion(JSON.parse(row.payload)));
    versions = records;
    const groups = new Map<string, DraftVersion[]>();
    for (const record of records) {
      const group = groups.get(record.session) ?? [];
      group.push(record);
      groups.set(record.session, group);
    }
    const next: Drafts = {};
    const alternatives: DraftVersion[] = [];
    for (const [session, group] of groups) {
      const active = activeDraftVersions(group);
      const own = draftVersionId(device, writer, session);
      const chosen =
        active.find((record) => record.id === own) ??
        active.find((record) => !dismissed.has(key(record)));
      if (chosen && next[session] !== chosen.text) {
        next[session] = chosen.text;
      }
      for (const record of active) {
        if (record.text && record.text !== next[session])
          alternatives.push(record);
        for (const text of record.alternatives ?? [])
          if (text && text !== next[session])
            alternatives.push({
              ...record,
              id: `${record.id}:conflict`,
              text,
            });
      }
    }
    const previous = current;
    current = next;
    publish(previous, next);
    const unique = new Set<string>();
    const nextConflicts = alternatives.filter((version) => {
      const id = key(version);
      if (dismissed.has(id) || unique.has(id)) return false;
      unique.add(id);
      return true;
    });
    setConflicts((previous) =>
      previous.length === nextConflicts.length &&
      previous.every((item, index) => key(item) === key(nextConflicts[index]!))
        ? previous
        : nextConflicts,
    );
  };
  const store = createDirectDrafts({
    post: (rows) => options.api.syncPost("/api/sync/drafts", { rows }),
    changed: reconcile,
    buffer: draftStore(options.serverId ?? serverViewId ?? "local"),
  });
  const flush = () => {
    clearTimeout(quiet);
    clearTimeout(maximum);
    quiet = maximum = undefined;
    void store
      .flush()
      .then(() =>
        setError((previous) => (previous === incompleteDrafts ? previous : "")),
      )
      .catch(() =>
        setError("Draft changes could not be saved. Keep this chat open."),
      );
  };
  const schedule = () => {
    clearTimeout(quiet);
    quiet = setTimeout(flush, DRAFT_SYNC_TIMING_MS.pushQuietWait);
    maximum ??= setTimeout(flush, DRAFT_SYNC_TIMING_MS.pushMaxWait);
  };
  const setDrafts = (value: Drafts | ((old: Drafts) => Drafts)) => {
    const previous = current;
    const next = typeof value === "function" ? value(previous) : value;
    current = next;
    publish(previous, next);
    const updated = Math.max(
      Date.now(),
      ...versions.map((version) => version.updated + 1),
    );
    for (const session of new Set([
      ...Object.keys(previous),
      ...Object.keys(next),
    ])) {
      if (previous[session] === next[session]) continue;
      const id = draftVersionId(device, writer, session);
      const seen = Object.fromEntries(
        versions
          .filter((version) => version.session === session)
          .flatMap((version) => [
            [version.id, version.updated],
            ...Object.entries(version.seen ?? {}),
          ])
          .sort((a, b) => Number(a[1]) - Number(b[1])),
      );
      const version: DraftVersion = {
        id,
        session,
        device,
        text: next[session] || "",
        updated,
        seen,
      };
      store.edit({
        id,
        payload: encodeDraftPayload(version),
        seq: 0,
        _deleted: false,
      });
      versions = [...versions.filter((item) => item.id !== id), version];
    }
    schedule();
  };
  const getDraft = (session: string) => current[session] || "";
  const subscribeDraft = (session: string, listener: () => void) => {
    const callbacks = listeners.get(session) ?? new Set<() => void>();
    callbacks.add(listener);
    listeners.set(session, callbacks);
    return () => {
      callbacks.delete(listener);
      if (!callbacks.size) listeners.delete(session);
    };
  };
  const dismissDraft = (version: DraftVersion) => {
    dismissed.add(key(version));
    reconcile();
  };
  const start = () => {
    const fail = () =>
      setUnsentError("Draft changes could not be saved. Keep this chat open.");
    if (typeof window !== "undefined")
      window.addEventListener(UNSENT_FAILURE_EVENT, fail);
    void store
      .restore()
      .then(schedule)
      .catch(() => setError("Draft changes could not be restored."));
    const stop = options.resources.watchResourceChanges(
      { kind: "drafts" },
      (version) => {
        if (!version) return;
        try {
          if (version.draftSnapshot) {
            store.snapshot(
              version.draftSnapshot.documents,
              version.draftSnapshot.complete,
              version.revision,
            );
            setError(version.draftSnapshot.complete ? "" : incompleteDrafts);
            schedule();
          } else if (
            version.reason === "change" &&
            version.revision > store.sequence()
          ) {
            const batch = version.draftChanges;
            if (
              !batch ||
              store.sequence() < 0 ||
              batch.after > store.sequence() ||
              batch.through !== version.revision ||
              batch.through <= batch.after ||
              batch.documents.at(-1)?.seq !== batch.through
            ) {
              store.waitForBaseline();
              options.resources.replaceStream();
              return;
            }
            store.changes(batch.documents, batch.through);
          }
        } catch {
          setError("Some saved drafts could not be read. Keep this chat open.");
        }
      },
      { waitForBaseline: true },
    );
    return () => {
      stop();
      if (typeof window !== "undefined")
        window.removeEventListener(UNSENT_FAILURE_EVENT, fail);
      clearTimeout(quiet);
      clearTimeout(maximum);
    };
  };
  return {
    setDrafts,
    getDraft,
    subscribeDraft,
    get conflicts() {
      return metadata.conflicts;
    },
    dismissDraft,
    get error() {
      return metadata.unsentError || metadata.error;
    },
    get localPersistenceFailed() {
      return (
        !!metadata.unsentError ||
        (!!metadata.error && metadata.error !== incompleteDrafts)
      );
    },
    start,
  };
}
