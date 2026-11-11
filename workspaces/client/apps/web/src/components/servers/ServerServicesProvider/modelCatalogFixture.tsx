import type { ComponentSnapshot as Snapshot } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";
import type { ReactNode } from "react";
import { observable, action, flow } from "mobx";
import { createModelsMirror } from "../../../generated/observableResources.ts";
import {
  ServerServicesProvider,
  useServerServices,
  useServerStore,
} from "./ServerServicesProvider.tsx";
import type { ServerServices } from "../../../servers/serverServices.ts";
import type { GetOptions, GetResult } from "../../../api.ts";

type Projection = Pick<Snapshot, "entitiesComplete"> & {
  runtime: Pick<Snapshot["runtime"], "modelsByAccount">;
};
export type ModelCatalogFixtureSource = {
  scope: string;
  cacheOwner?: object;
  read(
    options: Pick<GetOptions<"/api/models">, "query" | "signal">,
  ): Promise<GetResult<"/api/models">>;
  subscribeState(
    accept: (snapshot: Projection) => void,
    fail: (error: unknown | null) => void,
  ): () => void;
};
/** Browser entrypoints create the catalog before rendering and dispose it on teardown. */
export function createModelCatalogFixture(value: ModelCatalogFixtureSource) {
  const models = createModelsMirror();
  const ready = observable.box(false);
  const dispose = value.subscribeState(
    (snapshot) => {
      models.apply(
        Object.entries(snapshot.runtime.modelsByAccount ?? {}).flatMap(
          ([key, rows]) =>
            rows.map((row) => ({
              collection: "model" as const,
              id: JSON.stringify([key, row.model]),
              value: row,
            })),
        ),
      );
      action(() => ready.set(snapshot.entitiesComplete === true))();
    },
    () => {},
  );
  const modelReader = {
    scope: value.scope,
    read: flow(function* (options: Parameters<typeof value.read>[0]) {
      const result: Awaited<ReturnType<typeof value.read>> =
        yield value.read(options);
      if (options.query?.retry && result.data?.length) {
        const accountKey = options.query.account_key ?? "default";
        models.apply(
          result.data.map((row, catalogPosition) => ({
            collection: "model" as const,
            id: JSON.stringify([accountKey, row.model]),
            value: {
              ...row,
              accountKey,
              provider: row.provider ?? "codex",
              catalogPosition,
            },
          })),
        );
      }
      return result;
    }),
  };
  return { models, ready, modelReader, dispose };
}
export function ModelCatalogFixture({
  value,
  children,
}: {
  value: ReturnType<typeof createModelCatalogFixture>;
  children: ReactNode;
}) {
  const parent = useServerStore();
  const services = useServerServices();
  const { models, ready, modelReader } = value;
  const owner = {
    ...services,
    owner: services?.owner ?? {
      id: "fixture",
      origin: "",
      credentialId: null,
      label: "Fixture",
    },
    modelReader,
    store: {
      ...parent,
      models: () => ({
        get current() {
          return ready.get()
            ? { status: "live" as const, data: models.current }
            : { status: "loading" as const };
        },
      }),
    },
  } as ServerServices;
  return (
    <ServerServicesProvider services={owner}>{children}</ServerServicesProvider>
  );
}
