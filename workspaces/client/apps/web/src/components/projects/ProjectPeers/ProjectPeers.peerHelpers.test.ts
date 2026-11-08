import { describe, expect, it } from "vitest";
import {
  capabilityLabel,
  fingerprintOf,
  formatFingerprint,
  parseProjectInvitation,
} from "./peerHelpers.ts";

const invitation = {
  protocol: 1,
  inviteId: "invite-1",
  token: "token",
  stateId: "00000000-0000-4000-8000-000000000001",
  label: "Studio server",
  origin: "https://a.example.ts.net",
  publicKey: "PEM",
  expires: 1_900_000_000,
  scope: "project",
  projectId: "project-a",
};

describe("project peers", () => {
  it("accepts a project invitation and refuses server invitations", () => {
    expect(parseProjectInvitation(JSON.stringify(invitation))).toEqual(
      invitation,
    );
    expect(
      parseProjectInvitation(
        JSON.stringify({ ...invitation, scope: undefined }),
      ),
    ).toBeNull();
    expect(parseProjectInvitation("not json")).toBeNull();
    expect(parseProjectInvitation("[]")).toBeNull();
  });

  it("accepts an invitation without a Serve origin", () => {
    expect(
      parseProjectInvitation(JSON.stringify({ ...invitation, origin: null })),
    ).not.toBeNull();
  });

  it("groups the fingerprint in blocks of four", () => {
    expect(formatFingerprint("abcdef0123")).toBe("abcd ef01 23");
  });

  it("computes the SHA-256 hex that the server stores", async () => {
    // SHA-256 of the empty string.
    expect(await fingerprintOf("")).toBe(
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
  });

  it("names the lead messages capability in plain words", () => {
    expect(capabilityLabel("LeadMessages")).toContain("Lead messages");
  });
});
