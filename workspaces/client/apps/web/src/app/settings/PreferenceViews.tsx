import { NativeSelect, Slider, Switch, TextInput } from "@mantine/core";
import { observer } from "mobx-react-lite";
import type { ReactNode, ComponentProps } from "react";
import { SettingsRow } from "../../components/ui/primitives.tsx";
import Disclosure from "../../components/ui/Disclosure.tsx";
import ProviderUpdateSettings from "../../components/ProviderUpdateSettings.tsx";
import {
  fontFamilies,
  formatSidebarShortcut,
} from "../../studioPreferences.ts";
import type { useAppPreferences } from "./useAppPreferences.ts";
type PreferenceControls = ReturnType<typeof useAppPreferences>;

function Segments<Value extends string | number>({
  label,
  name,
  options,
  value,
  choose,
}: {
  label: string;
  name: string;
  options: Readonly<Record<string, Value>>;
  value: Value | null;
  choose: (value: Value) => void;
}) {
  return (
    <SettingsRow label={label}>
      <div
        className="studio-appearance-segments"
        role="radiogroup"
        aria-label={`Studio ${label.toLowerCase()}`}
      >
        {Object.entries(options).map(([text, option]) => (
          <label key={option}>
            <input
              type="radio"
              name={name}
              value={option}
              checked={value === option}
              onChange={() => choose(option)}
            />
            <span>{text}</span>
          </label>
        ))}
      </div>
    </SettingsRow>
  );
}
function Range({
  label,
  unit,
  name,
  children,
  ...props
}: Pick<
  ComponentProps<typeof Slider>,
  "value" | "disabled" | "min" | "max" | "onChange"
> & { label: string; unit: string; name: string; children?: ReactNode }) {
  return (
    <SettingsRow
      label={
        <span className="studio-range-label">
          {label} <output>{`${props.value}${unit}`}</output>
        </span>
      }
    >
      <div className="studio-range-field">
        <Slider {...props} thumbLabel={name} step={1} />
        {children}
      </div>
    </SettingsRow>
  );
}
export const AppearanceSettings = observer(function AppearanceSettings({
  children,
  p,
}: {
  children?: ReactNode;
  p: PreferenceControls;
}) {
  const v = p.value;
  return (
    <div className="studio-appearance-groups">
      <Segments
        label="Theme"
        name="studio-theme"
        options={{ System: "auto", Light: "light", Dark: "dark" }}
        value={v.theme}
        choose={(theme) => p.update({ theme })}
      />
      <ProviderUpdateSettings
        checked={v.checkProviderUpdates}
        onChange={(checked) => p.update({ checkProviderUpdates: checked })}
      />
      <SettingsRow label="Show terminals">
        <Switch
          aria-label="Show terminals"
          checked={v.showTerminals}
          onChange={(event) =>
            p.update({ showTerminals: event.currentTarget.checked })
          }
        />
      </SettingsRow>
      <Segments
        label="Text size"
        name="studio-text-size"
        options={{ S: 12, M: 14, L: 16 }}
        value={
          v.typography === "custom" && v.sidebarFontSize === v.mainFontSize
            ? v.mainFontSize
            : null
        }
        choose={(size) =>
          p.update({
            typography: "custom",
            sidebarFontSize: size,
            mainFontSize: size,
          })
        }
      />
      <Segments
        label="Chat width"
        name="studio-chat-width"
        options={{ Narrow: 60, Wide: 80, Full: 100 }}
        value={v.contentLayout === "custom" ? v.contentWidth : null}
        choose={(contentWidth) =>
          p.update({ contentLayout: "custom", contentWidth })
        }
      />
      <SettingsRow
        label="Hide old chats"
        help="Projects with fewer chats always show all of them."
      >
        <TextInput
          aria-label="Hide old chats"
          inputMode="numeric"
          pattern="[0-9]*"
          value={p.hideOldChatsThresholdText}
          onChange={(event) => p.editThreshold(event.currentTarget.value)}
          onBlur={p.commitThreshold}
        />
      </SettingsRow>
      <Disclosure className="studio-appearance-more" summary="More">
        <div className="studio-appearance-more-content">
          <FontSettings p={p} />
          <ChatLayoutSettings p={p} />
          {children}
        </div>
      </Disclosure>
    </div>
  );
});
export const FontSettings = observer(function FontSettings({
  p,
}: {
  p: PreferenceControls;
}) {
  const v = p.value;
  return (
    <section className="settings-group" aria-label="Fonts">
      <h2>Fonts</h2>
      <SettingsRow label="Text style">
        <NativeSelect
          aria-label="Studio text style"
          value={v.typography}
          data={[
            { value: "original", label: "Default text" },
            { value: "custom", label: "Custom text" },
          ]}
          onChange={(event) =>
            p.update({ typography: event.currentTarget.value })
          }
        />
        <small>Select Custom text to change the font and text sizes.</small>
      </SettingsRow>
      <SettingsRow label="Font family">
        <NativeSelect
          aria-label="Studio font family"
          disabled={v.typography !== "custom"}
          value={v.fontFamily}
          data={Object.entries(fontFamilies).map(([value, font]) => ({
            value,
            label: font.label,
          }))}
          onChange={(event) =>
            p.update({ fontFamily: event.currentTarget.value })
          }
        />
      </SettingsRow>
      {(
        [
          ["sidebarFontSize", "Sidebar"],
          ["mainFontSize", "Main"],
        ] as const
      ).map(([field, label]) => (
        <Range
          key={field}
          label={`${label} text`}
          name={`${label} font size`}
          unit="px"
          min={12}
          max={24}
          value={p.value[field]}
          disabled={v.typography !== "custom"}
          onChange={(value) => p.update({ [field]: value })}
        />
      ))}
    </section>
  );
});
export const ChatLayoutSettings = observer(function ChatLayoutSettings({
  p,
}: {
  p: PreferenceControls;
}) {
  const v = p.value;
  return (
    <>
      <section className="settings-group" aria-label="Chat layout">
        <h2>Chat layout</h2>
        <SettingsRow label="Width">
          <NativeSelect
            aria-label="Chat width layout"
            value={v.contentLayout}
            data={[
              { value: "original", label: "Default width" },
              { value: "custom", label: "Custom width" },
            ]}
            onChange={(event) =>
              p.update({ contentLayout: event.currentTarget.value })
            }
          />
          <small>Select Custom width to change the chat width.</small>
        </SettingsRow>
        <Range
          label="Custom width"
          unit="%"
          name="Transcript width"
          value={v.contentWidth}
          disabled={v.contentLayout !== "custom"}
          min={60}
          max={100}
          onChange={(value) => p.update({ contentWidth: value })}
        >
          <small>
            Sets the reading width of messages. The composer keeps its width.
            Narrow screens use the full width.
          </small>
        </Range>
      </section>
      <section className="settings-group" aria-label="Messages">
        <h2>Messages</h2>
        <label className="settings-field studio-preference-toggle">
          <span className="settings-label">Show message avatars</span>
          <input
            aria-label="Show message avatars"
            type="checkbox"
            checked={v.showMessageAvatars}
            onChange={(event) =>
              p.update({ showMessageAvatars: event.currentTarget.checked })
            }
          />
        </label>
      </section>
    </>
  );
});
export const HotkeySettings = observer(function HotkeySettings({
  p,
}: {
  p: PreferenceControls;
}) {
  const v = p.value;
  return (
    <section className="settings-group" aria-label="Sidebar shortcut">
      <h2>Keyboard shortcut</h2>
      <SettingsRow label="Toggle sidebar">
        <div className="studio-shortcut-control">
          <TextInput
            className="studio-shortcut-input"
            aria-describedby="studio-shortcut-hint"
            aria-label="Toggle sidebar shortcut"
            readOnly
            value={formatSidebarShortcut(v.sidebarShortcut)}
            onKeyDown={p.captureShortcut}
            onBlur={p.resetShortcutFeedback}
          />
          <div className="studio-shortcut-tokens" aria-hidden="true">
            {formatSidebarShortcut(v.sidebarShortcut)
              .split("+")
              .map((key, index) => (
                <kbd key={`${key}:${index}`}>{key}</kbd>
              ))}
          </div>
        </div>
        <small id="studio-shortcut-hint">
          Select the field. Press a modifier and a letter or number.
        </small>
        {p.shortcutFeedback && (
          <small role="status">{p.shortcutFeedback}</small>
        )}
      </SettingsRow>
    </section>
  );
});
