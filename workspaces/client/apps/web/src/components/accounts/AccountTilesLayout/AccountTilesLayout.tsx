import { ProviderMark } from "../ProviderMark/ProviderMark.tsx";
import { SegmentedControl } from "@mantine/core";
import type { ReactNode } from "react";

import { providerLabel } from "../../../generated/providerKind.ts";

/** Shared provider control and account grid for local account selection. */
export function AccountTilesLayout({
  label,
  provider,
  providers,
  onProvider,
  children,
}: {
  label: string;
  provider: string;
  providers: string[];
  onProvider: (provider: string) => void;
  children: ReactNode;
}) {
  return (
    <div className="account-tiles" role="group" aria-label={label}>
      {/* A choice of one provider is no choice; the tiles name it. */}
      {providers.length > 1 && (
        <SegmentedControl
          fullWidth
          aria-label={`${label} provider`}
          value={provider}
          onChange={onProvider}
          data={providers.map((value) => ({
            value,
            label: (
              <span className="setup-provider">
                <ProviderMark provider={value} />
                {providerLabel(value)}
              </span>
            ),
          }))}
        />
      )}
      {children}
    </div>
  );
}
