import { ProviderMark } from "../ProviderMark/ProviderMark.tsx";
import { SegmentedControl } from "@mantine/core";
import type { ReactNode, ComponentProps } from "react";

import { providerLabel } from "../../../generated/providerKind.ts";

/** Shared provider control and account grid for local account selection. */
export function AccountTilesLayout({
  label,
  provider,
  providers,
  onProvider,
  children,
  disabled,
  heading,
  showSingleProvider = false,
  ...props
}: {
  disabled?: boolean | undefined;
  heading?: ReactNode;
  showSingleProvider?: boolean;
  label: string;
  provider: string;
  providers: string[];
  onProvider: (provider: string) => void;
  children: ReactNode;
} & ComponentProps<"div">) {
  return (
    <div
      className="account-tiles"
      role="group"
      aria-label={label}
      {...props}
      aria-disabled={disabled}
    >
      {heading}
      {/* A choice of one provider is no choice; the tiles name it. */}
      {providers.length > (showSingleProvider ? 0 : 1) && (
        <SegmentedControl
          fullWidth
          disabled={!!disabled}
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
