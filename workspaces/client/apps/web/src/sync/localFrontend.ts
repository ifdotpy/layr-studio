import { hasNativeClient } from "../native/client";

const LOOPBACK_HOSTS = ["localhost", "127.0.0.1", "[::1]", "::1"];
/**
 * Native and loopback frontends do not persist subscription caches. A browser
 * using a LAN address may cache even when it runs on the server machine.
 */
export function frontendIsLocal(
  _owner: string,
  nativeClient: boolean = hasNativeClient(),
  hostname: string = globalThis.location?.hostname ?? "",
) {
  return (
    nativeClient ||
    LOOPBACK_HOSTS.includes(hostname) ||
    hostname.endsWith(".localhost")
  );
}
