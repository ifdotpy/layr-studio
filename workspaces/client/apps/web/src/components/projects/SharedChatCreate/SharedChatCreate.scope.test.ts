// @vitest-environment happy-dom
import { expect, it } from "vitest";
import { ServerAreas } from "../../../app/ServerAreasProvider.tsx";
import {
  sharedCreationKey,
  type SharedCreation,
} from "../../../app/projects/CreationOwner.ts";
import { createServerServices } from "../../../servers/serverServices.ts";
import {
  observableFixture,
  setStartupFixture,
} from "../../shell/integration-support/observableFixture.tsx";
it("persists a late receipt to its original scope without replacing a newer attempt", () => {
  const service = createServerServices({
      id: crypto.randomUUID(),
      origin: "https://fixture.invalid",
      credentialId: "fixture",
      label: "Fixture",
    }),
    models = observableFixture(),
    area = new ServerAreas({
      services: service,
      store: models.store,
      screen: { active: true, channel: null },
    });
  const body: SharedCreation["body"] = {
    action: "radio",
    radio_action: "create",
    request_id: "request-a",
    path: "/a",
    name: "Shared",
    participants: [],
  };
  try {
    setStartupFixture(area, { stateDir: "A" });
    area.creation.rememberShared({ body });
    setStartupFixture(area, { stateDir: "B" });
    const newer: SharedCreation = {
      body: { ...body, request_id: "request-b" },
    };
    area.creation.rememberShared(newer);
    const receipt: SharedCreation = { body, roomId: "room-a" };
    area.creation.rememberShared(receipt, "A");
    expect(service.api.saved(sharedCreationKey("A"), null)).toEqual(receipt);
    expect(service.api.saved(sharedCreationKey("B"), null)).toEqual(newer);
    expect(area.creation.sharedAttempt).toEqual(newer);
    area.creation.rememberShared(
      {
        ...newer,
        body: { ...body, request_id: "obsolete" },
        roomId: "obsolete-room",
      },
      "B",
    );
    expect(area.creation.sharedAttempt).toEqual(newer);
    expect(service.api.saved(sharedCreationKey("B"), null)).toEqual(newer);
  } finally {
    area.dispose();
    service.close();
    models.store.dispose();
  }
});
