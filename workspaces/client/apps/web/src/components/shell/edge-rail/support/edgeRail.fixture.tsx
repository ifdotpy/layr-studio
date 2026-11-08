import "../../../../rendererStyles.ts";
const fixtureRoot = document.getElementById("root");
if (!fixtureRoot) throw new Error("Missing fixture root");

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider, Menu } from "@mantine/core";
import { EdgeRail } from "/src/components/shell/edge-rail/EdgeRail/EdgeRail.tsx";
import { observeEdgeRail } from "/src/layout/useEdgeRail.ts";
import type { EdgeRailGeometry } from "../../../../layout/edgeRailPolicy.ts";
import { theme } from "/src/theme.ts";
import "@mantine/core/styles.css";
import "/src/layout/layout-contract.generated.css";
const e = React.createElement;
function Fixture() {
  const [geometry, setGeometry] = React.useState<EdgeRailGeometry | null>(null);
  React.useEffect(() => observeEdgeRail(setGeometry), []);
  const [last, setLast] = React.useState("None");
  return e(
    MantineProvider,
    { theme },
    e(
      "main",
      null,
      e("h1", null, "Conversation title"),
      e("output", { id: "last-action" }, last),
    ),
    e(EdgeRail, {
      geometry,
      sidebar: {
        expanded: true,
        onClick: () => setLast("Toggle conversations"),
      },
      messages: { count: 3, onClick: () => setLast("Messages") },
      menu: (
        <Menu.Item onClick={() => setLast("Existing action")}>
          Existing action
        </Menu.Item>
      ),
    }),
  );
}
createRoot(fixtureRoot).render(e(Fixture));
