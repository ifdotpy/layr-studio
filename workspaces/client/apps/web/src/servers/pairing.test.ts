import { afterAll, beforeAll, expect, it, vi } from "vitest";
import { identityProofMessage } from "../wasm/client";
import { readFileSync } from "node:fs";
import { createPublicKey, verify } from "node:crypto";

// The browser loads client_bg.wasm by URL. Node serves the same bytes from disk.
beforeAll(async () => {
  vi.stubGlobal(
    "fetch",
    async () =>
      new Response(
        readFileSync(
          new URL("../wasm/generated/client_bg.wasm", import.meta.url),
        ),
        { headers: { "Content-Type": "application/wasm" } },
      ),
  );
  // Load the module once, before any test fakes timers: the first load awaits a timer-based stream.
  await identityProofMessage("srv", "A".repeat(32));
});
afterAll(() => vi.unstubAllGlobals());
import {
  parseInvitation,
  pairedServer,
  signedRequest,
  publicKeyPem,
} from "./pairing";

// The golden vector of studio-request-signing (Rust). The runtime and client signers
// produce the same value for the same seed and request.
const GOLDEN_SIGNATURE =
  "PfjrkGA/IwuIryihDJjgbfNY/NicDM3N3JicKu0yQMHvEE58pUjBK502a+n1/jr/ovXn4d9nwTi1Q6p1nfX2Dg==";
const RFC_SEED = Buffer.from(
  "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
  "hex",
);
it("signs the exact encoded path, body and request identity without the local token", async () => {
  const keys = await crypto.subtle.generateKey({ name: "Ed25519" }, false, [
    "sign",
    "verify",
  ]);
  expect(keys.privateKey.extractable).toBe(false);
  const request = new Request(
    "https://computer.tailnet.ts.net/api/messages?q=a%20b",
    {
      method: "POST",
      body: '{"requestId":"once","text":"hello"}',
      headers: {
        "X-Canvas-Token": "local-secret",
        "X-Canvas-Workspace": "workspace",
      },
    },
  );
  const signed = await signedRequest(
    request,
    "server",
    "client",
    keys.privateKey,
    "once",
  );
  expect(signed.headers.get("X-Canvas-Token")).toBeNull();
  expect(signed.headers.get("X-Canvas-Workspace")).toBe("workspace");
  expect(signed.credentials).toBe("omit");
  expect(signed.redirect).toBe("error");
  const bytes = await request.clone().arrayBuffer();
  const hash = Buffer.from(
    await crypto.subtle.digest("SHA-256", bytes),
  ).toString("hex");
  const headers = signed.headers;
  const content = [
    "studio-multi-server-v1",
    "POST",
    "/api/messages?q=a%20b",
    "server",
    "client",
    headers.get("X-Studio-Timestamp")!,
    headers.get("X-Studio-Nonce")!,
    "once",
    hash,
  ].join("\n");
  const key = createPublicKey(
    publicKeyPem(await crypto.subtle.exportKey("spki", keys.publicKey)),
  );
  expect(
    verify(
      null,
      Buffer.from(content),
      key,
      Buffer.from(headers.get("X-Studio-Signature")!, "base64"),
    ),
  ).toBe(true);
  const retry = await signedRequest(
    request,
    "server",
    "client",
    keys.privateKey,
    "once",
  );
  expect(retry.headers.get("X-Studio-Nonce")).not.toBe(
    headers.get("X-Studio-Nonce"),
  );
  expect(await retry.text()).toBe(await signed.text());
});
it("signs the shared golden vector with the same bytes as the Rust signer", async () => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(1700000000 * 1000);
  // A zero random buffer gives the golden nonce: 24 zero bytes in unpadded base64url.
  const random = vi
    .spyOn(crypto, "getRandomValues")
    .mockImplementation(
      ((array: ArrayBufferView) => array) as typeof crypto.getRandomValues,
    );
  try {
    const privateKey = await crypto.subtle.importKey(
      "pkcs8",
      Buffer.concat([
        Buffer.from("302e020100300506032b657004220420", "hex"),
        RFC_SEED,
      ]),
      { name: "Ed25519" },
      false,
      ["sign"],
    );
    const request = new Request("https://server.example/api/x?q=1", {
      method: "POST",
      body: '{"a":1}',
    });
    const signed = await signedRequest(
      request,
      "server",
      "test-identity",
      privateKey,
      "request-1",
    );
    expect(signed.headers.get("X-Studio-Timestamp")).toBe("1700000000");
    expect(signed.headers.get("X-Studio-Nonce")).toBe(
      "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    );
    expect(signed.headers.get("X-Studio-Signature")).toBe(GOLDEN_SIGNATURE);
  } finally {
    random.mockRestore();
    vi.useRealTimers();
  }
});

it("pins pairing to the invitation's server identity, owner and public key", () => {
  const invitation = {
    protocol: 1 as const,
    inviteId: "invite",
    token: "code",
    serverId: "server-a",
    origin: "https://computer.tailnet.ts.net",
    label: "Computer",
    publicKey: "-----BEGIN PUBLIC KEY-----\nabc\n-----END PUBLIC KEY-----\n",
    tailscaleUser: "owner",
    expires: 1,
  };
  expect(
    parseInvitation(JSON.stringify({ invitation }), invitation.origin),
  ).toEqual(invitation);
  const receipt = { ...invitation, clientId: "client", paired: true as const };
  expect(pairedServer(receipt, invitation, "client").id).toBe("server-a");
  expect(() =>
    pairedServer({ ...receipt, publicKey: "other" }, invitation, "client"),
  ).toThrow();
  expect(() =>
    pairedServer({ ...receipt, tailscaleUser: "other" }, invitation, "client"),
  ).toThrow();
});

it("builds the identity proof message like the Rust owner", async () => {
  // Same vector as identity_proof_message_is_fixed_and_checks_the_nonce in Rust.
  expect(await identityProofMessage("srv", "A".repeat(32))).toBe(
    "studio-multi-server-v1\nidentity\nsrv\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
  );
});
