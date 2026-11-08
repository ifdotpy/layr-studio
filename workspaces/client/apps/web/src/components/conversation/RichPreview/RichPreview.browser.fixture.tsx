import "../../../rendererStyles.ts";
import { diagram, html } from "../support/RichPreview.browser.data.ts";

import { createElement as e } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "../../../browser-reset.css";
import "@mantine/core/styles.css";
import "../../../style.css";
import RichPreview from "./RichPreview.tsx";
const scheme =
  new URLSearchParams(location.search).get("scheme") === "dark"
    ? "dark"
    : "light";
const root = document.getElementById("root");
if (!root) throw new Error("Missing preview fixture root");
createRoot(root).render(
  e(
    MantineProvider,
    { forceColorScheme: scheme },
    e(
      "main",
      { style: { width: 1200 } },
      e(RichPreview, { kind: "mermaid", source: diagram }),
      e(RichPreview, { kind: "html", source: html }),
    ),
  ),
);
