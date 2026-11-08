import { resourceData } from "../../src/store/runtime.ts";
import { useEffect, useReducer, useState } from "react";
import { reaction } from "mobx";
import { StartupOwner } from "../../src/app/startup/StartupOwner.ts";
import { createServerViewServices } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ServerAreas } from "../../src/app/ServerAreasProvider.tsx";

/** Legacy recovery pages exercise the production startup lifecycle directly. */
export function useStartupFixture() {
  const [services] = useState(() =>
    createServerViewServices((scope) => new ServerAreas(scope)),
  );
  const [owner] = useState(
    () =>
      new StartupOwner(
        {
          services,
          store: services.store,
          screen: { active: false, channel: null },
        },
        { canUpdateRenderer: () => false, acceptUiPreferences: () => {} },
      ),
  );
  const [, render] = useReducer((version: number) => version + 1, 0);
  useEffect(() => {
    const stop = owner.start();
    const observe = reaction(
      () => [owner.isReady, owner.error, owner.workspaceId],
      () => render(),
      { fireImmediately: true },
    );
    return () => {
      observe();
      stop();
      services.close();
    };
  }, [owner]);
  return {
    startup: owner,
    api: services.api,
    get state() {
      return owner.isReady
        ? (resourceData(services.store.state().current) ?? null)
        : null;
    },
  };
}
