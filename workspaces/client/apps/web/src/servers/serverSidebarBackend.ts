import type { SidebarBackend } from "../components/sidebar/support/services.tsx";
import type {
  ApiPostPath,
  PostInput,
  PostOptions,
  PostResult,
} from "../api.ts";
import {
  readPreferenceFields,
  receivePreferenceFields,
  writePreferenceEdit,
} from "../sync/uiPreferenceStore.ts";
import type { ServerServices } from "./serverServices.ts";

export function serverSidebarBackend(
  services: ServerServices,
  online: () => boolean,
): SidebarBackend {
  const ownerId = services.owner.id;
  const storage = services.storage;
  return {
    ownerId,
    storage,
    resources: services.resources,
    async post<Path extends ApiPostPath>(
      path: Path,
      body: PostInput<Path>,
      options?: PostOptions,
    ): Promise<PostResult<Path>> {
      if (!online()) throw new Error("The sidebar server is offline.");
      await services.ready();
      return services.api.post(path, body, options);
    },
    saved<T>(key: string, fallback: T): T {
      try {
        const raw = storage.getItem(key);
        return raw === null ? fallback : JSON.parse(raw);
      } catch {
        return fallback;
      }
    },
    save(key, value) {
      storage.setItem(key, JSON.stringify(value));
    },
    editPreference(key, previous, next) {
      if (!online()) throw new Error("The sidebar server is offline.");
      return writePreferenceEdit(key, previous, next, storage);
    },
    preferenceFields: () => readPreferenceFields("server", storage),
    adoptPreferenceFields: (fields) =>
      receivePreferenceFields("server", fields, storage),
  };
}
