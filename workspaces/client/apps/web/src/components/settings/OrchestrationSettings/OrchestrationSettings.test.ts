import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { expect, test } from "vitest";
import { SettingsNumber } from "../SettingsNumber/SettingsNumber.tsx";
import {
  childResultHoldSpec,
  idleSessionUnloadSpec,
} from "./OrchestrationSettings.tsx";
import type { OrchestrationSettingSpec } from "../support/OrchestrationSettings.tsx";
import {
  CHILD_RESULT_COALESCE_DEFAULT_SECONDS,
  CHILD_RESULT_COALESCE_MAX_SECONDS,
  CHILD_RESULT_COALESCE_MIN_SECONDS,
  IDLE_SESSION_UNLOAD_DEFAULT_MINUTES,
  IDLE_SESSION_UNLOAD_MAX_MINUTES,
  IDLE_SESSION_UNLOAD_MIN_MINUTES,
} from "../../../generated/sharedValues.ts";

function render(spec: OrchestrationSettingSpec, value: number) {
  const { field: _field, ...input } = spec;
  return renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(SettingsNumber, {
        ...input,
        clampBehavior: "strict",
        value,
        disabled: false,
        onChange: () => undefined,
      }),
    ),
  );
}

test.each([
  [
    childResultHoldSpec,
    CHILD_RESULT_COALESCE_DEFAULT_SECONDS,
    CHILD_RESULT_COALESCE_MIN_SECONDS,
    CHILD_RESULT_COALESCE_MAX_SECONDS,
    "0 delivers every result immediately",
  ],
  [
    idleSessionUnloadSpec,
    IDLE_SESSION_UNLOAD_DEFAULT_MINUTES,
    IDLE_SESSION_UNLOAD_MIN_MINUTES,
    IDLE_SESSION_UNLOAD_MAX_MINUTES,
    "resumed automatically on the next message",
  ],
])(
  "renders %o with its tooltip and generated bounds",
  (spec, defaultValue, min, max, phrase) => {
    const markup = render(spec, defaultValue);
    expect(markup).toContain(`aria-label="${spec["aria-label"]}"`);
    expect(markup).toContain(`title="${spec.title.replaceAll("'", "&#x27;")}"`);
    expect(markup).toContain(`value="${defaultValue}"`);
    expect(spec.title).toContain(phrase);
    expect([spec.min, spec.max]).toEqual([min, max]);
    expect(max).toBeGreaterThan(defaultValue);
  },
);
