import { observable, action } from "mobx";
import type { SidebarBackend } from "../components/sidebar/support/services.tsx";
import type { MergedSidebar } from "./mergedSidebar.ts";
import type { ServerSidebarSnapshot } from "./sidebarSnapshot.ts";

type Override = {
  value: boolean | undefined;
  sources: Map<string, ServerSidebarSnapshot>;
  choice?:
    | {
        owner: string;
        name: string;
        kind: "compact" | "collapsed";
        snapshot: ServerSidebarSnapshot;
      }
    | undefined;
};

/** Session views do not change saved preference records. */
export function createSidebarVisualSession() {
  const values = observable.map<string, Map<string, Override>>(undefined, {
    deep: false,
  });
  return (
    model: MergedSidebar,
    backend: SidebarBackend,
    failed: (failure: unknown) => void,
  ): SidebarBackend => {
    const kindFor = (key: string) =>
      key === `codex-project-compact:${model.sidebar.stateDir}`
        ? "compact"
        : key === `codex-project-tree:${model.sidebar.stateDir}`
          ? "collapsed"
          : null;
    const read = <T>(key: string, fallback: T): T => {
      const base = backend.saved(key, fallback);
      const overrides = values.get(key);
      if (!overrides || !base || typeof base !== "object") return base;
      const result = { ...base } as Record<string, unknown>;
      for (const [field, override] of overrides) {
        const choice = override.choice;
        const source = choice && model.sourceById.get(choice.owner);
        if (
          choice &&
          source?.sidebar !== choice.snapshot &&
          source?.sidebar[choice.kind][choice.name] !== override.value
        ) {
          continue;
        }
        if (
          [...override.sources].some(
            ([owner, snapshot]) =>
              model.sourceById.get(owner)?.sidebar !== snapshot,
          )
        ) {
          continue;
        }
        if (override.value === undefined) delete result[field];
        else result[field] = override.value;
      }
      return result as T;
    };
    const owners = (kind: "compact" | "collapsed", field: string) => {
      const result = new Map<string, ServerSidebarSnapshot>();
      for (const source of model.sources) {
        const keys = model.preferenceKeys.get(source.id)![kind];
        if (
          [...keys].some(
            (key) =>
              (kind === "compact"
                ? model.projectKey(source.id, key)
                : model.mapTreeKey(source.id, key)) === field,
          )
        )
          result.set(source.id, source.sidebar);
      }
      return result;
    };
    return {
      ...backend,
      saved: read,
      viewPreference: action(
        <T extends Record<string, unknown>>(
          key: string,
          previous: T,
          next: T,
        ): T => {
          const overrides =
            values.get(key) ||
            observable.map<string, Override>(undefined, { deep: false });
          for (const field of new Set([
            ...Object.keys(previous),
            ...Object.keys(next),
          ]))
            if (previous[field] !== next[field])
              overrides.set(field, {
                value: next[field] as boolean | undefined,
                sources: new Map(),
              });
          values.set(key, overrides);
          return read(key, previous);
        },
      ),
      editPreference: action(
        <T extends Record<string, unknown>>(
          key: string,
          previous: T,
          next: T,
        ): T => {
          const kind = kindFor(key);
          const prior = previous as Record<string, boolean>;
          const value = next as Record<string, boolean>;
          const changed = kind
            ? [
                ...new Set([...Object.keys(prior), ...Object.keys(value)]),
              ].filter((field) => prior[field] !== value[field])
            : [];
          try {
            const accepted = backend.editPreference(key, previous, next);
            const overrides =
              values.get(key) ||
              observable.map<string, Override>(undefined, { deep: false });
            for (const field of changed) {
              const owner = kind && model.preferenceOwner(kind, field);
              const source = owner && model.sourceById.get(owner);
              const name =
                source &&
                kind &&
                [...model.preferenceKeys.get(source.id)![kind]].find(
                  (name) =>
                    (kind === "compact"
                      ? model.projectKey(source.id, name)
                      : model.mapTreeKey(source.id, name)) === field,
                );
              overrides.set(field, {
                value: value[field],
                sources: new Map(),
                choice:
                  source && kind && typeof name === "string"
                    ? { owner: source.id, name, kind, snapshot: source.sidebar }
                    : undefined,
              });
            }
            values.set(key, overrides);
            return read(key, accepted);
          } catch (failure) {
            if (
              kind &&
              failure instanceof Error &&
              failure.message === "The sidebar server is offline."
            ) {
              const overrides =
                values.get(key) ||
                observable.map<string, Override>(undefined, { deep: false });
              for (const field of changed)
                overrides.set(field, {
                  value: value[field],
                  sources: owners(kind, field),
                });
              values.set(key, overrides);
              return read(key, previous);
            }
            failed(failure);
            return previous;
          }
        },
      ),
    };
  };
}
