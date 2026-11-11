import { flowResult } from "mobx";
import { expect, it, vi } from "vitest";
import { ServerDetailsForm } from "./ServerDetailsForm.ts";

it("keeps an acknowledged name as the dirty baseline before the stream catches up", async () => {
  const form = new ServerDetailsForm();
  const saveName = vi.fn(async () => true);
  const input = {
    name: "First",
    alias: "ONE",
    usedAliases: [],
    saveName,
    saveAlias: vi.fn(async () => true),
  };
  form.name.change(input.name, "Second");
  await flowResult(form.save(input));
  expect(form.name.dirty(input.name)).toBe(false);
  form.name.change(input.name, "First");
  expect(form.name.dirty(input.name)).toBe(true);
  await flowResult(form.save(input));
  expect(saveName.mock.calls).toEqual([["Second"], ["First"]]);
});

it("lets a changed authoritative source replace an acknowledged value and an unsaved draft", async () => {
  const form = new ServerDetailsForm();
  const input = {
    name: "First",
    alias: "ONE",
    usedAliases: [],
    saveName: vi.fn(async () => true),
    saveAlias: vi.fn(async () => true),
  };
  form.alias.change("ONE", "TWO");
  await flowResult(form.save(input));
  form.alias.change("ONE", "THR");
  expect(form.alias.value("NEW")).toBe("NEW");
  expect(form.alias.dirty("NEW")).toBe(false);
  form.alias.change("NEW", "ONE");
  expect(form.alias.dirty("NEW")).toBe(true);
});
