import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import { useServerStore } from "../ServerServicesProvider/ServerServicesProvider.tsx";
import { AccessClientStatus } from "../../../generated/api.ts";
import { outboxRequests } from "../../../browserPersistence.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { useServerStorage } from "../ServerServicesProvider/ServerServicesProvider.tsx";
import { useServerApi } from "../ServerServicesProvider/ServerServicesProvider.tsx";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import { Button, SegmentedControl, Textarea, TextInput } from "@mantine/core";
import { useRef, useState } from "react";
import { errorText } from "../../../api.ts";
import type { ServerAccessRequest } from "../../../servers/accessContract.ts";
export default observer(function ServerAccessSettings({
  active,
}: {
  active: boolean;
}) {
  const outbox = outboxRequests(useServerStorage());
  const { serverAccess, refreshSession } = useServerApi();

  const store = useServerStore();
  const state = active ? resourceData(store.multiServer().current) : undefined;
  const [invitation, setInvitation] = useState("");
  const [invitationGrant, setInvitationGrant] = useState<"full" | "observe">(
    "full",
  );
  const [label, setLabel] = useState("Studio server");
  const [grant, setGrant] = useState<"full" | "observe">("full");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const pending = useRef<ServerAccessRequest | null>(readPending());
  function readPending(): ServerAccessRequest | null {
    try {
      return JSON.parse(outbox.get("serverAccess", "") || "null");
    } catch {
      return null;
    }
  }
  if (!active) return null;
  const run = (next: ServerAccessRequest) => {
    if (busy) return;
    if (
      pending.current &&
      JSON.stringify({ ...pending.current, requestId: "" }) !==
        JSON.stringify({ ...next, requestId: "" })
    ) {
      setError(
        "Retry the saved server access request before you start another request.",
      );
      return;
    }
    const body = pending.current || next;
    try {
      outbox.set("serverAccess", "", JSON.stringify(body));
      pending.current = body;
    } catch {
      setError("The request could not be saved. No server access changed.");
      return;
    }
    setBusy(true);
    setError("");
    void refreshSession()
      .then(() => serverAccess("POST", body))
      .then(async (result) => {
        if ("invitation" in result) {
          setInvitation(JSON.stringify(result.invitation, null, 2));
          setInvitationGrant(
            "grant" in body && body.grant === "observe" ? "observe" : "full",
          );
        }
        pending.current = null;
        outbox.remove("serverAccess", "");
      })
      .catch((failure: unknown) => setError(errorText(failure)))
      .finally(() => setBusy(false));
  };
  return (
    <section className="settings-group" aria-label="Server access">
      <h2>UI access to this server</h2>
      <p>Pair your other Studio UI with this server through Tailscale Serve.</p>
      {state && (
        <p>
          {state.identity.label} ({state.identity.origin})
        </p>
      )}
      <TextInput
        label="Server name"
        value={label}
        maxLength={80}
        onChange={(event) => setLabel(event.currentTarget.value)}
      />
      <SegmentedControl
        aria-label="Access of the paired UI"
        value={grant}
        disabled={busy}
        onChange={(value) => setGrant(value === "observe" ? "observe" : "full")}
        data={[
          { value: "full", label: "Full access" },
          { value: "observe", label: "View only" },
        ]}
      />
      <ActionButton
        actionRole="secondary"
        disabled={busy}
        onClick={() =>
          run({
            action: "create_invite",
            label,
            grant,
            requestId: randomUUID(),
          })
        }
      >
        Create pairing invitation
      </ActionButton>
      {pending.current && (
        <Button
          disabled={busy}
          variant="subtle"
          onClick={() => run(pending.current!)}
        >
          Retry saved request
        </Button>
      )}
      {invitation && (
        <Textarea
          label="Pairing invitation"
          value={invitation}
          readOnly
          autosize
          minRows={6}
          description={
            invitationGrant === "observe"
              ? "Copy the whole invitation into the other UI. The paired UI can view this server but cannot change anything."
              : "Copy the whole invitation into the other UI. The invitation grants full server access."
          }
        />
      )}
      {error && <p role="alert">{error}</p>}
      {state?.clients.map((client) => (
        <div key={client.clientId}>
          <strong>{client.label}</strong>
          <Button
            disabled={busy || client.status === AccessClientStatus.revoked}
            variant="subtle"
            onClick={() =>
              run({
                action: "revoke",
                clientId: client.clientId,
                requestId: randomUUID(),
              })
            }
          >
            Revoke access
          </Button>
        </div>
      ))}
    </section>
  );
});
