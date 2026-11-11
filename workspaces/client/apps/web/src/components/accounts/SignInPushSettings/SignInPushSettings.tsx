import { observer } from "mobx-react-lite";
import { useServerServices } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import * as localApi from "../../../servers/net/api.ts";
import { SignInPush } from "./SignInPush.ts";
import { Button } from "@mantine/core";
import { pushSupported } from "../../../push/signInPush.ts";
const localPush = new SignInPush(localApi);
export const SignInPushSettings = observer(function SignInPushSettings({
  active,
}: {
  active: boolean;
}) {
  const flow = useServerServices()?.areas.signInPush ?? localPush;
  if (!active) return null;
  const { state, error, busy, fingerprint, thisDevice } = flow;
  return (
    <section className="settings-group" aria-label="Phone sign-in alerts">
      <h2>Sign-in alerts on the phone</h2>
      <p>
        When a sign-in card waits for you, Studio sends one alert to each device
        below, and one reminder before the card closes. The alert shows only the
        sign-in host and the chat name.
      </p>
      {!pushSupported() && (
        <p>
          This window cannot get push alerts. Try adding Studio to your home
          screen and opening it from there.
        </p>
      )}
      {state && !state.ready && (
        <p role="status">
          Studio sends push alerts only when remote access has an https address.
        </p>
      )}
      {state && pushSupported() && (
        <Button
          mt="md"
          loading={busy}
          disabled={!state.ready || !!thisDevice || fingerprint === undefined}
          onClick={() => void flow.enable(state.publicKey)}
        >
          {thisDevice
            ? "This device gets alerts"
            : "Send alerts to this device"}
        </Button>
      )}
      {state && state.subscriptions.length > 0 && (
        <ul aria-label="Devices with sign-in alerts">
          {state.subscriptions.map((device) => (
            <li key={device.id}>
              {device.id === thisDevice
                ? `${device.device} (this device)`
                : device.device}
              , since {new Date(device.created * 1000).toLocaleDateString()}{" "}
              <Button
                size="compact-sm"
                variant="subtle"
                disabled={
                  busy || (pushSupported() && fingerprint === undefined)
                }
                onClick={() => void flow.stop(device.id)}
              >
                Stop alerts
              </Button>
            </li>
          ))}
        </ul>
      )}
      {error && <p role="alert">{error}</p>}
    </section>
  );
});

export { SignInPush } from "./SignInPush.ts";
