import {
  NativeAccountState as GeneratedApiNativeAccountState,
  NativeCandidateState as GeneratedApiNativeCandidateState,
  NativeRuntimeState as GeneratedApiNativeRuntimeState,
  ProviderVersionState as GeneratedApiProviderVersionState,
} from "../generated/api.ts";
import { ProviderKind } from "../generated/providerKind.ts";
import Disclosure from "./ui/Disclosure";
import { useServerStore } from "../servers/ServerServicesContext";
import { localDateTime } from "../local-time";
import { observer } from "mobx-react-lite";
import { resourceData } from "../store/runtime.ts";
import { errorText } from "../api";
import type { Account } from "./Accounts";
import ProviderUpdateNotices from "./ProviderUpdateNotices";
import "./native-runtime-status.css";

const accountStatus = {
  current: "Up to date",
  waiting: "Waiting",
  updating: "Updating",
  failed: "Update failed",
};

const providerStatus = {
  checking: "Checking",
  current: "Running version meets the tested baseline",
  outdated: "Below the tested baseline",
  unknown: "Running version unknown",
  error: "Version check failed",
};

export default observer(function NativeRuntimeStatus({
  opened,
  accounts,
}: {
  opened: boolean;
  accounts: readonly Account[];
}) {
  const store = useServerStore();
  const current = opened ? store.desktop().current : undefined;
  const desktop = current ? resourceData(current) : undefined;
  const data = desktop?.nativeRuntime ?? null;
  const providers = desktop?.providerVersions ?? {
    checkedAt: null,
    providers: [],
    warnings: [],
  };
  const error = current?.status === "error" ? errorText(current.error) : "";

  if (!opened) return null;
  const entries = Object.entries(data?.accounts || {});
  const pending = entries.filter(
    ([, account]) =>
      account.status === GeneratedApiNativeAccountState.waiting ||
      account.status === GeneratedApiNativeAccountState.updating,
  ).length;
  const reasonCount = (reason: string) =>
    entries.filter(([, runtime]) => runtime.reason === reason).length;
  const rejected = data?.candidates.filter(
    (candidate) =>
      candidate.status === GeneratedApiNativeCandidateState.rejected,
  );
  return (
    <section
      className="accounts-runtime"
      aria-label="Runtime and provider versions"
    >
      <h3>Runtime and provider versions</h3>
      {(error ||
        data?.status === GeneratedApiNativeRuntimeState.failed ||
        data?.error ||
        rejected?.length ||
        entries.some(
          ([, runtime]) =>
            runtime.status === GeneratedApiNativeAccountState.failed,
        ) ||
        providers.providers.some(
          (provider) =>
            provider.status === GeneratedApiProviderVersionState.error ||
            provider.status === GeneratedApiProviderVersionState.outdated ||
            provider.status === GeneratedApiProviderVersionState.unknown ||
            !!provider.error,
        )) && (
        <p role="status" className="native-runtime-error">
          Some runtime checks need attention. See the details below.
        </p>
      )}
      <Disclosure summary="Version details">
        {data && (
          <section className="native-runtime-status" aria-label="Codex runtime">
            <div className="native-runtime-heading">
              <strong>Codex runtime</strong>
              {data.selected && <span>{data.selected.version}</span>}
            </div>
            <p className="native-runtime-summary" role="status">
              {data.status === GeneratedApiNativeRuntimeState.checking
                ? "Checking installed versions…"
                : data.status === GeneratedApiNativeRuntimeState.failed
                  ? "Runtime check failed"
                  : "Automatic updates"}
              {pending > 0 &&
                ` · ${pending} account${pending === 1 ? "" : "s"} pending`}
            </p>
            {data.checkedAt != null && data.checkedAt > 0 && (
              <time dateTime={new Date(data.checkedAt * 1000).toISOString()}>
                Checked {localDateTime(new Date(data.checkedAt * 1000))}
              </time>
            )}
            {data.error && <p className="native-runtime-error">{data.error}</p>}
            {entries.length > 0 && (
              <ul className="native-runtime-accounts">
                {entries.map(([key, runtime]) => {
                  const account = accounts.find((item) => item.id === key);
                  return (
                    <li key={key}>
                      <div className="native-runtime-account-heading">
                        <span>{account?.label || account?.email || key}</span>
                        <span
                          className={
                            runtime.status ===
                            GeneratedApiNativeAccountState.failed
                              ? "native-runtime-error"
                              : undefined
                          }
                        >
                          {accountStatus[runtime.status]}
                          {runtime.version && ` · ${runtime.version}`}
                        </span>
                      </div>
                      {runtime.reason && reasonCount(runtime.reason) === 1 && (
                        <small>{runtime.reason}</small>
                      )}
                      {runtime.targetVersion &&
                        runtime.status !==
                          GeneratedApiNativeAccountState.current && (
                          <small>Target: {runtime.targetVersion}</small>
                        )}
                    </li>
                  );
                })}
              </ul>
            )}
            {[
              ...new Set(
                entries
                  .map(([, runtime]) => runtime.reason)
                  .filter(
                    (reason): reason is string =>
                      !!reason && reasonCount(reason) > 1,
                  ),
              ),
            ].map((reason) => (
              <p key={reason}>{reason}</p>
            ))}
            {rejected?.map((candidate) => (
              <p
                className="native-runtime-error"
                key={`${candidate.path}:${candidate.version}`}
              >
                Version {candidate.version || "unknown"} rejected
                {candidate.error ? `: ${candidate.error}` : "."}
              </p>
            ))}
          </section>
        )}
        <section
          className="native-runtime-status provider-version-status"
          aria-label="Provider versions"
        >
          <div className="native-runtime-heading">
            <strong>Provider CLI versions</strong>
            {providers.checkedAt != null && providers.checkedAt > 0 && (
              <time
                dateTime={new Date(providers.checkedAt * 1000).toISOString()}
              >
                Checked {localDateTime(new Date(providers.checkedAt * 1000))}
              </time>
            )}
          </div>
          {error && <p className="native-runtime-error">{error}</p>}
          {providers.providers.length === 0 ? (
            <p className="native-runtime-summary" role="status">
              {providers.checkedAt == null
                ? "Checking connected provider CLIs…"
                : "No connected provider CLIs were found."}
            </p>
          ) : (
            <ul className="native-runtime-accounts">
              {providers.providers.map((provider) => {
                const account = accounts.find(
                  (item) => item.id === provider.accountKey,
                );
                const label =
                  provider.provider === ProviderKind.Claude
                    ? "Claude Code"
                    : provider.provider === ProviderKind.Codex
                      ? "Codex CLI"
                      : provider.provider;
                const updateNotices = providers.warnings.filter(
                  (warning) => warning.accountKey === provider.accountKey,
                );
                return (
                  <li key={provider.id}>
                    <div className="native-runtime-account-heading">
                      <span>
                        {label} ·{" "}
                        {account?.label ||
                          account?.email ||
                          provider.accountKey}
                      </span>
                      <span
                        className={
                          provider.status ===
                          GeneratedApiProviderVersionState.error
                            ? "native-runtime-error"
                            : provider.status ===
                                GeneratedApiProviderVersionState.outdated
                              ? "native-runtime-warning"
                              : undefined
                        }
                      >
                        {providerStatus[provider.status]}
                      </span>
                    </div>
                    <ProviderUpdateNotices notices={updateNotices} />
                    <small>
                      Running version: {provider.runningVersion || "Unknown"}
                    </small>
                    <small>
                      Installed version:{" "}
                      {provider.installedVersion || "Unknown"}
                    </small>
                    {provider.configuredVersion &&
                      provider.configuredVersion !==
                        provider.installedVersion && (
                        <small>
                          Current profile executable:{" "}
                          {provider.configuredVersion}
                        </small>
                      )}
                    <small>
                      Tested minimum: {provider.baseline || "Not established"}
                    </small>
                    {provider.error && (
                      <small className="native-runtime-error">
                        {provider.error}
                      </small>
                    )}
                    {provider.message &&
                      provider.status !==
                        GeneratedApiProviderVersionState.current && (
                        <small
                          className={
                            provider.status ===
                            GeneratedApiProviderVersionState.outdated
                              ? "native-runtime-warning"
                              : undefined
                          }
                        >
                          {provider.message}
                        </small>
                      )}
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      </Disclosure>
    </section>
  );
});
