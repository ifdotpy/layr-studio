import { flowResult } from "mobx";
import { expect, it, vi } from "vitest";
import type { PostResult } from "../../../api.ts";
import { FederationSettingsStore } from "./FederationSettingsStore.ts";

type Reply = PostResult<"/api/federation">;
const invitation = {
  protocol: 1,
  inviteId: "invite",
  token: "token",
  stateId: "peer",
  label: "Peer",
  origin: "https://peer.example",
  publicKey: "key",
  expires: 1,
} satisfies Extract<Reply, { invitation: unknown }>["invitation"];
const created = { invitation, expires: 1, warning: "" } satisfies Reply;
const snapshot = {
  enabled: true,
  identity: null,
  invites: [],
  queued: 0,
  version: 1,
  peers: [],
  rooms: [],
} satisfies Reply;

it("creates an invitation and pairs the current draft through the server owner", async () => {
  const post = vi
    .fn()
    .mockResolvedValueOnce(created)
    .mockResolvedValueOnce(snapshot);
  const notify = vi.fn(),
    change = vi.fn();
  const model = new FederationSettingsStore({ post }, notify);
  try {
    await flowResult(model.invite("Current label", "", change));
    expect(post).toHaveBeenNthCalledWith(1, "/api/federation", {
      action: "create_invite",
      label: "Current label",
      expected_user: undefined,
    });
    expect(change).toHaveBeenLastCalledWith(
      JSON.stringify(invitation, null, 2),
    );
    await flowResult(model.pair(JSON.stringify(invitation), change));
    expect(post).toHaveBeenNthCalledWith(2, "/api/federation", {
      action: "accept_peer",
      invitation,
    });
    expect(change).toHaveBeenLastCalledWith("");
    expect(notify).not.toHaveBeenCalled();
  } finally {
    model.dispose();
  }
});

it("refuses malformed pairing drafts without sending a request", async () => {
  const post = vi.fn(),
    notify = vi.fn(),
    change = vi.fn();
  const model = new FederationSettingsStore({ post }, notify);
  try {
    await flowResult(model.pair("not JSON", change));
    await flowResult(
      model.pair(JSON.stringify({ ...invitation, protocol: 2 }), change),
    );
    expect(post).not.toHaveBeenCalled();
    expect(change).not.toHaveBeenCalled();
    expect(notify).toHaveBeenCalledTimes(2);
  } finally {
    model.dispose();
  }
});

it("does not publish a pending invitation after its owner cancels", async () => {
  const reply = Promise.withResolvers<Reply>();
  const change = vi.fn();
  const model = new FederationSettingsStore(
    { post: vi.fn().mockReturnValue(reply.promise) },
    vi.fn(),
  );
  try {
    const pending = flowResult(model.invite("Label", "expected-user", change));
    model.cancel();
    reply.resolve(created);
    await pending;
    expect(change).not.toHaveBeenCalled();
  } finally {
    model.dispose();
  }
});
