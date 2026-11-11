import { Button, SegmentedControl, Textarea, TextInput } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { AccessClientStatus } from "../../../generated/api.ts";
import { resourceData } from "../../../store/runtime.ts";
import { textField } from "../../../stores/fields.ts";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import {
  useServerApi,
  useServerStorage,
  useServerStore,
} from "../ServerServicesProvider/ServerServicesProvider.tsx";
import { ServerAccessForm } from "./ServerAccessForm.ts";
export default observer(function ServerAccessSettings({
  active,
}: {
  active: boolean;
}) {
  const storage = useServerStorage();
  const api = useServerApi();
  const store = useServerStore();
  const state = active ? resourceData(store.multiServer().current) : undefined;
  return (
    <Owned create={() => new ServerAccessForm()}>
      {(form) =>
        active && (
          <section className="settings-group" aria-label="Server access">
            <h2>UI access to this server</h2>
            <p>
              Pair your other Studio UI with this server through Tailscale
              Serve.
            </p>
            {state && (
              <p>
                {state.identity.label} ({state.identity.origin})
              </p>
            )}
            <TextInput
              label="Server name"
              {...textField(form, "label")}
              maxLength={80}
            />
            <SegmentedControl
              aria-label="Access of the paired UI"
              value={form.grant}
              disabled={form.busy}
              onChange={form.setGrant}
              data={[
                { value: "full", label: "Full access" },
                { value: "observe", label: "View only" },
              ]}
            />
            <ActionButton
              actionRole="secondary"
              disabled={form.busy}
              onClick={() => void form.createInvitation(api, storage)}
            >
              Create pairing invitation
            </ActionButton>
            {form.readPending(storage) && (
              <Button
                disabled={form.busy}
                variant="subtle"
                onClick={() => void form.retry(api, storage)}
              >
                Retry saved request
              </Button>
            )}
            {form.invitation?.via === api && (
              <Textarea
                label="Pairing invitation"
                value={form.invitation.value}
                readOnly
                autosize
                minRows={6}
                description={
                  form.invitation.grant === "observe"
                    ? "Copy the whole invitation into the other UI. The paired UI can view this server but cannot change anything."
                    : "Copy the whole invitation into the other UI. The invitation grants full server access."
                }
              />
            )}
            {form.error && <p role="alert">{form.error}</p>}
            {state?.clients.map((client) => (
              <div key={client.clientId}>
                <strong>{client.label}</strong>
                <Button
                  disabled={
                    form.busy || client.status === AccessClientStatus.revoked
                  }
                  variant="subtle"
                  onClick={() =>
                    void form.revoke(client.clientId, api, storage)
                  }
                >
                  Revoke access
                </Button>
              </div>
            ))}
          </section>
        )
      }
    </Owned>
  );
});
