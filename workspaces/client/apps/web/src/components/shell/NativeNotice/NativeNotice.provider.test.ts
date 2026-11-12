import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { Message } from "../../../types.ts";
import { NativeNotice } from "./NativeNotice.tsx";

it.each([
  "The provider configuration changed.",
  "This model setting is deprecated.",
  "Model changed from gpt-original to gpt-routed (additional safety checks).",
  "Changed from claude-opus-4-6 to claude-sonnet-4-6 after refusal.",
])("renders the provider notice: %s", (text) => {
  const item = {
    id: "provider-notice",
    role: "system",
    text,
    nativeNotice: "warning",
  } satisfies Message;
  const markup = renderToStaticMarkup(createElement(NativeNotice, { item }));
  expect(markup).toContain(text);
});
