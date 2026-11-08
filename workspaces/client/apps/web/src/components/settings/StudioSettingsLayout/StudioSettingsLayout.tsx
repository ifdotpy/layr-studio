import { studioSettingsTabs } from "../support/StudioSettingsTabs.tsx";
import StudioSettingsTabs from "../StudioSettingsTabs/StudioSettingsTabs.tsx";
import { Button, Tabs } from "@mantine/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";

/** One section navigation shared by the local and multi-server settings. */
export function StudioSettingsLayout({
  value,
  onChange,
  children,
  initialDetail = false,
}: {
  value: StudioSettingsTab;
  onChange: (value: string | null) => void;
  children: ReactNode;
  initialDetail?: boolean | undefined;
}) {
  const [detail, setDetail] = useState(initialDetail || value !== "accounts");
  const heading = useRef<HTMLHeadingElement>(null);
  const navigation = useRef<HTMLDivElement>(null);
  const title = studioSettingsTabs.find(([id]) => id === value)?.[1];
  useEffect(() => {
    if (detail && heading.current?.getClientRects().length)
      heading.current.focus({ preventScroll: true });
  }, [detail, value]);
  return (
    <Tabs
      value={value}
      onChange={onChange}
      orientation="vertical"
      variant="pills"
      className="studio-settings-tabs"
      // Hidden settings tabs unmount so they release their live subscriptions.
      keepMounted={false}
      data-detail={detail || undefined}
    >
      <div className="studio-settings-navigation" ref={navigation}>
        <StudioSettingsTabs onSelect={() => setDetail(true)} />
      </div>
      <div className="studio-settings-content">
        <header className="studio-settings-detail-heading">
          <Button
            variant="subtle"
            leftSection={<ArrowLeft aria-hidden="true" />}
            onClick={() => {
              setDetail(false);
              requestAnimationFrame(() =>
                navigation.current
                  ?.querySelector<HTMLButtonElement>('[data-active="true"]')
                  ?.focus({ preventScroll: true }),
              );
            }}
          >
            Back
          </Button>
          <h2 ref={heading} tabIndex={-1}>
            {title}
          </h2>
        </header>
        {children}
      </div>
    </Tabs>
  );
}

export type StudioSettingsTab = (typeof studioSettingsTabs)[number][0];
