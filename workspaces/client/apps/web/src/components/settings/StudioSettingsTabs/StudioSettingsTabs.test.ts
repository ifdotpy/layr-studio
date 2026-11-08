import { MantineProvider, Tabs } from "@mantine/core";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { StudioSettingsLayout } from "../StudioSettingsLayout/StudioSettingsLayout.tsx";

const render = (initialDetail?: boolean) =>
  renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(StudioSettingsLayout, {
        value: "accounts",
        onChange: () => {},
        initialDetail,
        children: createElement(Tabs.Panel, {
          value: "accounts",
          children: "Saved accounts",
        }),
      }),
    ),
  );

it("starts normal Accounts navigation at the section list", () => {
  expect(render()).not.toContain("data-detail=");
});

it("restores the Accounts detail before an account dialog returns focus", () => {
  const html = render(true);
  expect(html).toContain('data-detail="true"');
  expect(html).toContain("Saved accounts");
});
