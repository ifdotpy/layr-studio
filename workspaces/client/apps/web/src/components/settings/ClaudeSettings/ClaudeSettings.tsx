import { ClaudeSettingsView } from "../ClaudeSettingsView/ClaudeSettingsView.tsx";
import { observer } from "mobx-react-lite";
import "../support/claude-settings.css";

export const ClaudeSettings = observer(ClaudeSettingsView);
