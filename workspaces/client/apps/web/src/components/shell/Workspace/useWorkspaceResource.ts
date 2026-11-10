import { useEffect, useMemo } from "react";
import type { ApiGetPath, GetOptions } from "../../../api.ts";
import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { WorkspaceResource } from "./WorkspaceResource.ts";

type Options<P extends ApiGetPath> = Pick<GetOptions<P>, "query">;
type Args<P extends ApiGetPath> =
  {} extends Options<P> ? [options?: Options<P>] : [options: Options<P>];

export function useWorkspaceResource<P extends ApiGetPath>(
  path: P,
  revision: string | number,
  ...args: Args<P>
) {
  const { get } = useServerApi();
  const query = args[0]?.query;
  const key = JSON.stringify(query ?? {});
  const state = useMemo(
    () =>
      new WorkspaceResource((signal, etag, readMetadata) =>
        get(path, { query, signal, etag, readMetadata }),
      ),
    [get, path, key],
  );
  useEffect(() => state.start(), [state]);
  useEffect(() => {
    void state.load();
  }, [state, revision]);
  return state;
}
