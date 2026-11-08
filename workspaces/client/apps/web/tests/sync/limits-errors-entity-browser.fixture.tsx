import { fixtureElement } from "./browser-helpers.ts";
import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import App from "../../src/App.tsx";
import {
  ServerAreas,
  ServerAreasProvider,
} from "../../src/app/ServerAreasProvider.tsx";
import {
  createServerViewServices,
  ServerServicesProvider,
} from "../../src/servers/ServerServicesContext.tsx";
const services = createServerViewServices((scope) => new ServerAreas(scope));
import { SidebarWarningsProvider } from "../../src/components/SidebarWarnings.tsx";
import { theme } from "../../src/theme.ts";
import "../../src/style.css";
createRoot(fixtureElement("root")).render(
  <MantineProvider theme={theme}>
    <SidebarWarningsProvider>
      <ServerServicesProvider services={services}>
        <ServerAreasProvider areas={services.areas}>
          <App />
        </ServerAreasProvider>
      </ServerServicesProvider>
    </SidebarWarningsProvider>
  </MantineProvider>,
);
