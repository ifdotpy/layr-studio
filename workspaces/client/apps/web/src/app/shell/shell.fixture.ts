import { setStartupFixture } from "../../components/shell/integration-support/observableFixture.tsx";
import { runInAction } from "mobx";
import { createServerServices } from "../../servers/serverServices.ts";
import { observableFixture } from "../../components/shell/integration-support/observableFixture.tsx";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import { componentSnapshot as snapshotFixture } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { AgentSource } from "../../generated/api.ts";
import { ServerAreas } from "../ServerAreasProvider.tsx";

let serial = 0;
export function shellFixture(mobile = false) {
  const services = createServerServices({
    id: `shell-fixture-${++serial}`,
    origin: "https://shell.test",
    credentialId: "test",
    label: "Shell",
  });
  services.api.save("codex-confirmed-chats", {
    scope: "workspace",
    agents: [agentSummaryFixture("confirmed")],
  });
  const models = observableFixture([
    { id: "lead", source: AgentSource.managed, isLead: true, updated: 1 },
  ]);
  const areas = new ServerAreas({
    services,
    store: models.store,
    screen: { active: true, channel: null },
  });
  runInAction(() => {
    setStartupFixture(areas, {
      ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
      stateDir: "workspace",
    });
  });
  const width = mobile ? 600 : 1800;
  areas.layout.recordResize({
    host: { x: 0, y: 0, width, height: 800 },
    viewport: { x: 0, y: 0, width, height: 800 },
    rootFontPx: 16,
    shortTouchLandscape: false,
  });
  return {
    areas,
    models,
    services,
    dispose() {
      areas.dispose();
      services.close();
      models.store.dispose();
    },
  };
}
