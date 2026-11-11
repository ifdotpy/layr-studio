import { FullOutputs } from "../../support/FullOutputs.ts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { makeAutoObservable } from "mobx";
import { expect, it, vi } from "vitest";
import FullOutput from "./FullOutput.tsx";
vi.mock("mobx", async (original) => {
  const actual = await original<typeof import("mobx")>();
  return { ...actual, makeAutoObservable: vi.fn(actual.makeAutoObservable) };
});
it("an unobserved render allocates no observable output state", () => {
  const owner = new FullOutputs();
  vi.mocked(makeAutoObservable).mockClear();
  const markup = renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(FullOutput, {
        owner,
        blob: { hash: "unobserved-hash", size: 1024 },
      }),
    ),
  );
  expect(markup).toContain("Open full output");
  expect(makeAutoObservable).not.toHaveBeenCalled();
  expect(owner.current.size).toBe(0);
});
