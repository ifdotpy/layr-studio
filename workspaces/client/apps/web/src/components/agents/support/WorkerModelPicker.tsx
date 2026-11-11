import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import type { ServerServices } from "../../../servers/serverServices.ts";
import {
  useServerStore,
  useServerModelReader,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { flow, makeAutoObservable, computed, compareStructural } from "mobx";
import { resourceData } from "../../../store/runtime.ts";
import type { DeepReadonly } from "../../../deepReadonly.ts";
import { ApiError, get, errorText } from "../../../servers/net/api.ts";
import { serverResourceScope } from "../../../servers/registry.ts";

import type { components } from "../../../generated/api.ts";
import type { KnownApiFields } from "../../../apiTypes.ts";

type ProviderModel = KnownApiFields<components["schemas"]["ProviderModel"]>;
export type WorkerModelInfo = DeepReadonly<
  ProviderModel & {
    serviceTiers: NonNullable<ProviderModel["serviceTiers"]>;
    supportedReasoningEfforts: NonNullable<
      ProviderModel["supportedReasoningEfforts"]
    >;
  }
>;

const modelInfo = (value: DeepReadonly<ProviderModel>): WorkerModelInfo => ({
  ...value,
  serviceTiers: value.serviceTiers ?? [],
  supportedReasoningEfforts: value.supportedReasoningEfforts ?? [],
});

export const isDaybreakAlias = (model: string) =>
  /^gpt-daybreak-(blue|red)-latest$/.test(model);

const cyberAccessPrograms = (
  info?: WorkerModelInfo,
): readonly string[] | null | undefined => {
  if (!info || !Object.hasOwn(info, "availableAccessPrograms"))
    return undefined;
  const access = info.availableAccessPrograms;
  if (access === null) return null;
  return access?.cyber;
};

export function daybreakProgram(info?: WorkerModelInfo): string | null {
  const programs = cyberAccessPrograms(info);
  if (!programs) return null;
  if (programs.includes("daybreakBlue")) return "daybreakBlue";
  if (programs.includes("daybreakRed")) return "daybreakRed";
  return null;
}

export function supportsDaybreakMode(
  info: WorkerModelInfo | undefined,
  enabled: boolean,
) {
  if (!info || isDaybreakAlias(info.model)) return false;
  const programs = cyberAccessPrograms(info);
  if (enabled) return !!daybreakProgram(info);
  return (
    programs === undefined ||
    (programs !== null && programs.includes("standard"))
  );
}

type Store = ReturnType<typeof useServerStore>;
export type ModelCatalogSource = ServerServices["modelReader"];
const localReader: ModelCatalogSource = {
  scope: serverResourceScope(),
  read: (options) => get("/api/models", options),
};
export function useModelCatalogReader() {
  return useServerModelReader() ?? localReader;
}

/** Fresh metadata is an explicit read, distinct from the streamed display catalog. */
export async function readWorkerModels(
  reader: ModelCatalogSource,
  accountKey: string,
  workers = false,
): Promise<WorkerModelInfo[]> {
  const data = await reader.read({
    query: {
      account_key: accountKey,
      workers: workers ? "1" : undefined,
      fresh: "1",
    },
  });
  return (data.data ?? []).map(modelInfo);
}
export const workerCatalog = createWorkerCatalog;
export function createWorkerCatalog(
  store: Store,
  reader: ModelCatalogSource,
  accountKey: string,
  workers = false,
) {
  return makeAutoObservable(
    {
      active: true,
      pending: false,
      retryError: "",
      dispose() {
        this.active = false;
      },
      get error() {
        const current = store.models().current;
        return (
          this.retryError ||
          ("error" in current ? errorText(current.error) : "")
        );
      },

      get models(): WorkerModelInfo[] {
        const data = resourceData(store.models().current);
        let rows = [...(data?.modelsByAccount.values() ?? [])].filter(
          (row) =>
            !row.hidden &&
            !!row.model &&
            (workers || row.accountKey === accountKey),
        );
        const rank = (key: string) =>
          key === accountKey ? 0 : key === "default" ? 1 : 2;
        rows.sort(
          (a, b) =>
            (workers ? rank(a.accountKey) - rank(b.accountKey) : 0) ||
            a.catalogPosition - b.catalogPosition ||
            a.accountKey.localeCompare(b.accountKey),
        );
        if (workers) {
          const seen = new Set<string>();
          rows = rows.filter((row) => {
            if (seen.has(row.model)) return false;
            seen.add(row.model);
            return true;
          });
        }
        return rows.map(modelInfo);
      },
      get loading() {
        return (
          !resourceData(store.models().current) ||
          (this.pending && !this.models.length)
        );
      },
      retry: flow(function* (this: {
        active: boolean;
        pending: boolean;
        retryError: string;
      }): Generator<
        ReturnType<ModelCatalogSource["read"]>,
        void,
        Awaited<ReturnType<ModelCatalogSource["read"]>>
      > {
        if (!this.active || this.pending) return;
        this.pending = true;
        this.retryError = "";
        try {
          yield reader.read({
            query: {
              account_key: accountKey,
              workers: workers ? "1" : undefined,
              retry: "1",
            },
          });
          if (this.active) this.retryError = "";
        } catch (error) {
          if (this.active)
            this.retryError =
              error instanceof ApiError &&
              error.status === 503 &&
              error.catalogPending
                ? ""
                : errorText(error);
        } finally {
          this.pending = false;
        }
      }),
    },
    { models: computed({ equals: compareStructural }) },
    { autoBind: true },
  );
}
export const emptyCatalog = {
  models: [] as WorkerModelInfo[],
  loading: false,
  error: "",
  retry: () => {},
};
export const loadingCatalog = { ...emptyCatalog, loading: true };
export const workerCatalogKey = (accountKey: string, workers = false) =>
  JSON.stringify([accountKey, workers]);
export function useWorkerModels(
  accountKey: string,
  enabled: boolean,
  workers = false,
) {
  const owner = useServerAreas().agentsUi;
  return enabled
    ? (owner.workerCatalogs.get(workerCatalogKey(accountKey, workers)) ??
        loadingCatalog)
    : emptyCatalog;
}
