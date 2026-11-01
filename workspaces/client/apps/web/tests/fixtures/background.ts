import type { components } from "../../src/generated/api.ts";

export function backgroundDisplay(
  kind: "command" | "monitor" | "backgroundTask" = "command",
  title = "Fixture command",
): components["schemas"]["BackgroundTaskDisplay"] {
  return {
    kind,
    kindLabel: kind === "backgroundTask" ? "tool" : kind,
    title,
    active: true,
    inBackgroundList: true,
    indicator: "working",
    label: "running",
    actions:
      kind === "backgroundTask"
        ? ["readOutput", "stop"]
        : ["readOutput", "input", "resize", "stop"],
  };
}
