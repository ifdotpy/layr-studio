/** Pure helpers of the project "Peers" section (decision 2026-11-01, "Pairing per project"). */

export type PeerInvitation = {
  protocol: 1;
  inviteId: string;
  token: string;
  stateId: string;
  label: string;
  leadName?: string | null;
  origin?: string | null;
  publicKey: string;
  expires: number;
  scope: "project";
  projectId: string;
};

/** The capability names shown to the owner. */
export function capabilityLabel(capability: string): string {
  return capability === "LeadMessages"
    ? "Lead messages: the paired leads send each other messages"
    : capability;
}

/** Group a SHA-256 hex fingerprint in blocks of four for reading aloud. */
export function formatFingerprint(hex: string): string {
  return hex.match(/.{1,4}/g)?.join(" ") ?? hex;
}

/** The SHA-256 hex of a public key PEM, the same value the server stores as fingerprint. */
export async function fingerprintOf(pem: string): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(pem),
  );
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

/** Parse pasted invitation JSON. Returns null when the text is not a project invitation. */
export function parseProjectInvitation(text: string): PeerInvitation | null {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return null;
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const invitation = value as Record<string, unknown>;
  const valid =
    invitation.protocol === 1 &&
    invitation.scope === "project" &&
    typeof invitation.inviteId === "string" &&
    typeof invitation.token === "string" &&
    typeof invitation.stateId === "string" &&
    typeof invitation.label === "string" &&
    typeof invitation.publicKey === "string" &&
    typeof invitation.projectId === "string" &&
    typeof invitation.expires === "number" &&
    (invitation.origin === undefined ||
      invitation.origin === null ||
      typeof invitation.origin === "string");
  return valid ? (invitation as PeerInvitation) : null;
}
