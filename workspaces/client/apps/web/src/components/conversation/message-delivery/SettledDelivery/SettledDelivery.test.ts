import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import SettledDelivery from "./SettledDelivery.tsx";

const render = (transient: boolean) =>
  renderToStaticMarkup(
    createElement(SettledDelivery, {
      transient,
      children: createElement("span", null, "Status"),
    }),
  );

it("shows an outcome at once", () => {
  expect(render(false)).toBe("<span>Status</span>");
});

it("holds back a transient step on its first frame", () => {
  expect(render(true)).toBe("");
});
