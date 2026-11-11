// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import {
  PeerStatus,
  RoomStatus,
  type components,
} from "../../../generated/api.ts";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import {
  FederationSettings,
  FederationSettingsStore,
} from "./FederationSettings.tsx";

const api = vi.hoisted(() => ({ post: vi.fn() }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({ useServerServices: () => null }),
);

it("shares membership controls across room editors, retains opt-in on lead changes, and reads status only on open", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  Object.defineProperty(document, "fonts", {
    value: new EventTarget(),
    configurable: true,
  });
  const state = {
    enabled: true,
    identity: null,
    invites: [],
    queued: 0,
    version: 1,
    peers: [
      {
        created: null,
        label: "Peer",
        lastError: null,
        localApproved: true,
        origin: "peer",
        publicKey: "key",
        remoteApproved: true,
        stateId: "peer",
        status: PeerStatus.approved,
        updated: null,
        whoisStatus: null,
        whoisUser: null,
      },
    ],
    rooms: [
      {
        created: 1,
        id: "room",
        localMembers: [],
        name: "Room",
        peerId: "peer",
        peerLabel: "Peer",
        remoteMembers: [],
        shareNames: false,
        shareStatus: false,
        status: RoomStatus.pending,
      },
    ],
  } satisfies components["schemas"]["FederationSnapshot"];
  api.post.mockResolvedValue(state);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host),
    notify = vi.fn();
  const agents = [
    agentSummaryFixture("lead", { isLead: true, name: "Lead" }),
    agentSummaryFixture("worker", {
      rootId: "lead",
      name: "Worker",
      isLead: false,
    }),
    agentSummaryFixture("other", { isLead: true, name: "Other" }),
  ];
  const model = new FederationSettingsStore(api, notify);
  const render = (active: boolean, leadId = "lead") =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          {},
          createElement(FederationSettings, {
            active,
            leadId,
            agents,
            notify,
            model,
          }),
        ),
      ),
    );
  const inputs = (text: string) =>
    [...host.querySelectorAll("label")]
      .filter((label) => label.textContent === text)
      .map((label) =>
        host.querySelector<HTMLInputElement>("#" + CSS.escape(label.htmlFor))!,
      );
  try {
    await render(false);
    expect(api.post).not.toHaveBeenCalled();
    await render(true);
    expect(api.post).toHaveBeenCalledExactlyOnceWith("/api/federation", {
      action: "status",
    });
    expect(inputs("Worker")).toHaveLength(2);
    expect(inputs("Worker").every((input) => input.disabled)).toBe(true);
    await act(async () => inputs("Share names of selected agents")[0]!.click());
    expect(
      inputs("Share names of selected agents").every((input) => input.checked),
    ).toBe(true);
    await act(async () => inputs("Worker")[0]!.click());
    expect(inputs("Worker").every((input) => input.checked)).toBe(true);
    const invite = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Invite Peer to a room",
    );
    await act(async () => invite!.click());
    expect(api.post).toHaveBeenLastCalledWith("/api/federation", {
      action: "create_room",
      peer_id: "peer",
      local_members: ["lead", "worker"],
      share_names: true,
      share_status: false,
    });
    await render(true, "other");
    expect(inputs("Other (lead)").every((input) => input.checked)).toBe(true);
    expect(
      inputs("Share names of selected agents").every((input) => input.checked),
    ).toBe(true);
    expect(api.post).toHaveBeenCalledTimes(2);
    await render(false, "other");
    await render(true, "other");
    expect(api.post).toHaveBeenCalledTimes(3);
  } finally {
    await act(async () => root.unmount());
    model.dispose();
    host.remove();
    Reflect.deleteProperty(document, "fonts");
    vi.unstubAllGlobals();
    api.post.mockReset();
  }
});
