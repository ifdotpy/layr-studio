import { setStartupFixture } from "../../components/observableFixture.tsx";
import { createSidebarViewModel } from "../../components/sidebar/viewModel.ts";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { observable, reaction, runInAction } from "mobx";
import { ApiError, NetworkTimeoutError } from "../../api.ts";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import { fixturePostResult } from "../../../tests/misc-fixture.ts";

const cleanup: (() => void)[] = [];
afterEach(() => {
  cleanup
    .splice(0)
    .reverse()
    .forEach((stop) => stop());
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
import { ServerAreas } from "../ServerAreasProvider.tsx";
import { createServerServices } from "../../servers/serverServices.ts";
import { observableFixture } from "../../components/observableFixture.tsx";
import { componentSnapshot as snapshotFixture } from "../../components/componentTestFixtures.ts";
function fixture() {
  const services = createServerServices({
    id: `creation-${crypto.randomUUID()}`,
    origin: "https://fixture.invalid",
    credentialId: "fixture",
    label: "Fixture",
  });
  const models = observableFixture();
  const areas = new ServerAreas({
    services,
    store: models.store,
    screen: { active: true, channel: null },
  });
  const state = observable({ scope: "workspace-a" });
  const syncScope = reaction(
    () => state.scope,
    (scope) => {
      setStartupFixture(areas, {
        ...Object.assign(snapshotFixture(), {
          token: "",
          stateDir: "/fixture",
        }),
        stateDir: scope,
      });
    },
    { fireImmediately: true },
  );
  vi.spyOn(areas, "refresh").mockResolvedValue();
  const owner = areas.creation,
    chatCreations = services.chatCreations,
    mirror = models.mirror;
  const post = vi.spyOn(services.api, "post");
  cleanup.push(() => {
    owner.dispose();
    syncScope();
    models.store.dispose();
    services.close();
  });
  const request = {
    id: "created",
    previous: null,
    reuse_empty: false,
    cwd: "/project",
  };
  chatCreations.saveChatCreation(owner.creationKey, request);
  const receipt = fixturePostResult<"/api/leads">({
    id: "created",
    name: "Confirmed chat",
    projectPath: "/project",
  });
  return {
    owner,
    state,
    mirror,
    post,
    receipt,
    request,
    chatCreations,
    areas,
    confirmed: owner,
  };
}

it("retries a timeout with the exact request and body, then bridges delayed replication with one generated row", async () => {
  const f = fixture();
  const dispose = f.owner.start();
  f.post
    .mockRejectedValueOnce(new NetworkTimeoutError())
    .mockResolvedValueOnce(f.receipt);
  expect(await f.owner.retry(f.request)).toBe("created");
  expect(f.post).toHaveBeenCalledTimes(2);
  expect(f.post.mock.calls[0]?.[1]).toBe(f.post.mock.calls[1]?.[1]);
  expect(
    f.confirmed.unreplicatedCreatedAgents.map((agent) => agent.id),
  ).toEqual(["created"]);
  expect(f.areas.navigation.selectedId).toBe("created");
  expect(f.owner.pendingCreationRequests).toEqual([]);
  f.mirror.apply([
    {
      collection: "agentSummary",
      id: "created",
      value: agentSummaryFixture("created", { name: "Replicated chat" }),
    },
  ]);
  const generated = f.mirror.current.agents.get("created");
  expect(f.confirmed.lookup("created")).toBe(generated);
  const sidebar = createSidebarViewModel(
    () => f.mirror.current,
    () => ({ query: "", archived: false, opened: null, rank: () => 0 }),
    undefined,
    () => f.confirmed.unreplicatedCreatedAgents,
  );
  expect(sidebar.rows.map((row) => row.agent)).toEqual([generated]);
  f.confirmed.acknowledgeReplication();
  expect(f.confirmed.unreplicatedCreatedAgents).toEqual([]);
  dispose();
  vi.unstubAllGlobals();
});

it("clears only the refused request and preserves another project request", async () => {
  const f = fixture();
  f.chatCreations.saveChatCreation(f.owner.creationKey, {
    ...f.request,
    id: "other",
    cwd: "/other",
  });
  f.post.mockRejectedValueOnce(new ApiError("Refused", 409));
  expect(await f.owner.retry(f.request)).toBeNull();
  expect(f.owner.pendingCreationRequests.map((request) => request.id)).toEqual([
    "other",
  ]);
});

it("keeps navigation away during a create while remembering its confirmed row", async () => {
  const f = fixture();
  const result = Promise.withResolvers<typeof f.receipt>();
  f.post.mockReturnValueOnce(result.promise);
  const creating = f.owner.retry(f.request);
  runInAction(() => {
    f.areas.navigation.navigationIntent++;
    f.areas.navigation.selectedId = "away";
  });
  result.resolve(f.receipt);
  await creating;
  expect(f.areas.navigation.selectedId).toBe("away");
  expect(f.confirmed.lookup("created")?.name).toBe("Confirmed chat");
});

it("settles the old durable receipt but cannot mutate another server owner after disposal", async () => {
  const f = fixture();
  const result = Promise.withResolvers<typeof f.receipt>();
  f.post.mockReturnValueOnce(result.promise);
  const creating = f.owner.retry(f.request);
  f.owner.dispose();
  const other = fixture();
  other.mirror.apply([
    {
      collection: "agentSummary",
      id: "created",
      value: agentSummaryFixture("created", { name: "Other server" }),
    },
  ]);
  runInAction(() => {
    f.state.scope = "workspace-b";
  });
  result.resolve(f.receipt);
  expect(await creating).toBeNull();
  expect(f.areas.navigation.selectedId).toBeNull();
  expect(f.confirmed.lookup("created")).toBeUndefined();
  expect(other.owner.lookup("created")?.name).toBe("Other server");
  expect(other.areas.navigation.selectedId).toBeNull();

  expect(
    f.chatCreations.pendingChatCreations("codex-pending-creation:workspace-a"),
  ).toEqual([]);
});

it("loads durable requests when bootstrap publishes its actual scope and stops its listener on disposal", () => {
  const f = fixture();
  const events = new EventTarget();
  vi.stubGlobal("window", events);
  runInAction(() => {
    f.state.scope = "";
  });
  try {
    const dispose = f.owner.start();
    expect(f.owner.pendingCreationRequests).toEqual([]);
    runInAction(() => {
      f.state.scope = "workspace-a";
    });
    expect(f.owner.pendingCreationRequests).toEqual([f.request]);
    dispose();
  } finally {
    vi.unstubAllGlobals();
  }
});

it("settles an old A to B to A receipt without publishing its stale UI completion", async () => {
  const f = fixture();
  f.owner.start();
  const response = Promise.withResolvers<typeof f.receipt>();
  f.post.mockReturnValueOnce(response.promise);
  const creating = f.owner.retry(f.request);
  runInAction(() => {
    f.state.scope = "workspace-b";
  });
  runInAction(() => {
    f.state.scope = "workspace-a";
  });
  response.resolve(f.receipt);
  expect(await creating).toBeNull();
  expect(f.areas.navigation.selectedId).toBeNull();
  expect(f.owner.lookup("created")).toBeUndefined();
  expect(
    f.chatCreations.pendingChatCreations("codex-pending-creation:workspace-a"),
  ).toEqual([]);
});
