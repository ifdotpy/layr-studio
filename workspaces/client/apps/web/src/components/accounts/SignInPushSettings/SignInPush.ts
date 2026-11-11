import { whileObserved } from "../../../observed.ts";
import { flowResult, makeAutoObservable } from "mobx";
import { errorText, type GetResult, type ServerApi } from "../../../api.ts";
import {
  endpointFingerprint,
  enableSignInPush,
  pushSupported,
  stopSignInPush,
} from "../../../push/signInPush.ts";

async function deviceFingerprint() {
  const registration = await navigator.serviceWorker.getRegistration("/");
  const subscription = await registration?.pushManager.getSubscription();
  return subscription ? endpointFingerprint(subscription.endpoint) : null;
}
type Reading = [GetResult<"/api/web-push">, string | null | undefined];

/** Device subscription facts load only while the settings view observes them. */
export class SignInPush {
  state: GetResult<"/api/web-push"> | null = null;
  error = "";
  busy = false;
  fingerprint: string | null | undefined;
  readonly #api: ServerApi;
  constructor(api: ServerApi) {
    this.#api = api;
    makeAutoObservable(this, {}, { autoBind: true });
    whileObserved(this, "state", () => {
      void this.load();
      return () => {};
    });
  }
  get thisDevice() {
    return this.state?.subscriptions.find(
      (device) => device.endpointFingerprint === this.fingerprint,
    )?.id;
  }
  *load(): Generator<Promise<Reading>, void, Reading> {
    try {
      [this.state, this.fingerprint] = yield Promise.all([
        this.#api.get("/api/web-push"),
        pushSupported() ? deviceFingerprint() : undefined,
      ]);
      this.error = "";
    } catch (failure) {
      this.error = errorText(failure);
    }
  }
  *run(
    work: () => Promise<string | null | undefined>,
  ): Generator<Promise<unknown>, void, string | null | undefined> {
    if (this.busy) return;
    this.busy = true;
    this.error = "";
    try {
      this.fingerprint = yield work();
      yield flowResult(this.load());
    } catch (failure) {
      this.error = errorText(failure);
    } finally {
      this.busy = false;
    }
  }
  enable(publicKey: string) {
    const old = this.thisDevice;
    return this.run(async () => {
      const registration = await navigator.serviceWorker.ready;
      if (old)
        await stopSignInPush(
          await registration.pushManager.getSubscription(),
          () => this.#api.post("/api/web-push/remove", { id: old }),
        );
      await enableSignInPush({
        registration,
        requestPermission: () => Notification.requestPermission(),
        publicKey,
        userAgent: navigator.userAgent,
        save: (body) => this.#api.post("/api/web-push/subscriptions", body),
      });
      return deviceFingerprint();
    });
  }
  stop(id: string) {
    const local = id === this.thisDevice;
    return this.run(async () => {
      const registration = local
        ? await navigator.serviceWorker.getRegistration("/")
        : undefined;
      await stopSignInPush(
        (await registration?.pushManager.getSubscription()) ?? null,
        () => this.#api.post("/api/web-push/remove", { id }),
      );
      return local ? null : this.fingerprint;
    });
  }
}
