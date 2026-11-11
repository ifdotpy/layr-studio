import {
  NativeAccountState,
  NativeCandidateState,
  NativeRuntimeState,
  ProviderVersionState,
} from "../../../generated/api.ts";
import { ProviderKind } from "../../../generated/providerKind.ts";
import Disclosure from "../../ui/Disclosure/Disclosure.tsx";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { localDateTime } from "../../../local-time.ts";
import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import { errorText } from "../../../api.ts";
import type { Account } from "../../accounts/support/Accounts.tsx";
import ProviderUpdateNotices from "../../shell/ProviderUpdateNotices/ProviderUpdateNotices.tsx";

function CheckedAt({ value }: { value: number | null | undefined }) {
  return value != null && value > 0 ? (
    <time dateTime={new Date(value * 1000).toISOString()}>
      Checked {localDateTime(new Date(value * 1000))}
    </time>
  ) : null;
}

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
      account.status === NativeAccountState.waiting ||
      account.status === NativeAccountState.updating,
  ).length;
  const reasonCount = (reason: string) =>
    entries.filter(([, runtime]) => runtime.reason === reason).length;
  const rejected = data?.candidates.filter(
    (candidate) => candidate.status === NativeCandidateState.rejected,
  );
  return (
    <section
      className="accounts-runtime"
      aria-label="Runtime and provider versions"
    >
      <h3>Runtime and provider versions</h3>
      {(error ||
        data?.status === NativeRuntimeState.failed ||
        data?.error ||
        rejected?.length ||
        entries.some(
          ([, runtime]) => runtime.status === NativeAccountState.failed,
        ) ||
        providers.providers.some(
          (provider) =>
            provider.status === ProviderVersionState.error ||
            provider.status === ProviderVersionState.outdated ||
            provider.status === ProviderVersionState.unknown ||
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
              {data.status === NativeRuntimeState.checking
                ? "Checking installed versions…"
                : data.status === NativeRuntimeState.failed
                  ? "Runtime check failed"
                  : "Automatic updates"}
              {pending > 0 &&
                ` · ${pending} account${pending === 1 ? "" : "s"} pending`}
            </p>
            <CheckedAt value={data.checkedAt} />
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
                            runtime.status === NativeAccountState.failed
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
                        runtime.status !== NativeAccountState.current && (
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
            <CheckedAt value={providers.checkedAt} />
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
                          provider.status === ProviderVersionState.error
                            ? "native-runtime-error"
                            : provider.status === ProviderVersionState.outdated
                              ? "native-runtime-warning"
                              : undefined
                        }
                      >
                        {providerStatus[provider.status]}
                      </span>
                    </div>
                    <ProviderUpdateNotices notices={updateNotices} />
                    {Object.entries({
                      "Running version": provider.runningVersion || "Unknown",
                      "Installed version":
                        provider.installedVersion || "Unknown",
                      "Current profile executable":
                        provider.configuredVersion !==
                          provider.installedVersion &&
                        provider.configuredVersion,
                      "Tested minimum": provider.baseline || "Not established",
                    }).map(
                      ([label, value]) =>
                        value && (
                          <small key={label}>
                            {label}: {value}
                          </small>
                        ),
                    )}
                    {(
                      [
                        [provider.error, "native-runtime-error"],
                        [
                          provider.status !== ProviderVersionState.current &&
                            provider.message,
                          provider.status === ProviderVersionState.outdated
                            ? "native-runtime-warning"
                            : undefined,
                        ],
                      ] as const
                    ).map(
                      ([message, className], index) =>
                        message && (
                          <small key={index} className={className}>
                            {message}
                          </small>
                        ),
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
