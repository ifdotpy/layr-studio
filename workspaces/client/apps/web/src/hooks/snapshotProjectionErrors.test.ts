// @vitest-environment happy-dom
import { expect, it } from "vitest";
import { runInAction } from "mobx";
import { shellFixture } from "../app/shell/shell.fixture.ts";
it.each(["startupError", "projectionError"] as const)(
  "clears the %s alert after successful recovery",
  (field) => {
    const f = shellFixture(),
      owner = f.areas.startup;
    try {
      runInAction(() => {
        owner[field] = "Projection unavailable";
      });
      expect(owner.error).toBe("Projection unavailable");
      runInAction(() => {
        owner[field] = "";
      });
      expect(owner.error).toBe("");
    } finally {
      f.dispose();
    }
  },
);
