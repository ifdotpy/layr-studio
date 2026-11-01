import { AccessClientStatus } from "../generated/api";
import { expect, it } from "vitest";
import { accountServers } from "./accountServers";
import type { DiscoveredServer } from "./discoveryModel";

function peer(
  id: string,
  status: DiscoveredServer["status"],
): DiscoveredServer {
  return {
    id,
    label: id,
    origin: `https://${id}.tailnet.ts.net`,
    status,
    paired: status === AccessClientStatus.paired,
    lastSeen: null,
    publicKey: "public-key",
    generation: "1",
  };
}

it("only lists servers connected to this UI for account actions", () => {
  const registered = {
    ...peer("remote", AccessClientStatus.paired),
    credentialId: "ui-credential",
  };
  const rows = accountServers(
    [
      { id: "local", label: "This computer", origin: "http://localhost" },
      registered,
    ],
    [
      peer("remote", AccessClientStatus.paired),
      peer("backend", AccessClientStatus.paired),
      peer("offline", AccessClientStatus.unreachable),
      peer("new", AccessClientStatus.discovered),
      peer("revoked", AccessClientStatus.revoked),
    ],
  );
  expect(rows.map((row) => row.id)).toEqual(["local", "remote"]);
  expect(rows[1]).toBe(registered);
});

it("removes a backend-revoked peer even if this UI still has its credential", () => {
  const registered = {
    ...peer("remote", AccessClientStatus.paired),
    credentialId: "ui-credential",
  };
  expect(
    accountServers([registered], [peer("remote", AccessClientStatus.revoked)]),
  ).toEqual([]);
});

it("keeps the standalone local and manually paired UI server list", () => {
  const registered = [
    { id: "local", label: "This computer", origin: "http://localhost" },
    {
      ...peer("manual", AccessClientStatus.paired),
      credentialId: "ui-credential",
    },
  ];
  expect(accountServers(registered, [])).toEqual(registered);
});
