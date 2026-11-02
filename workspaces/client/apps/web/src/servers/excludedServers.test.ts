import { expect, it } from "vitest";
import { excludedServers, setServerExcluded } from "./excludedServers";

function memory(initial?: string) {
  const values = new Map<string, string>(
    initial === undefined ? [] : [["studio-automatic-ui-removals-v1", initial]],
  );
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
  };
}

it("keeps a removed server out until it is added again", () => {
  const storage = memory();
  setServerExcluded("remote", true, storage);
  expect([...excludedServers(storage)]).toEqual(["remote"]);
  setServerExcluded("remote", false, storage);
  expect(excludedServers(storage).has("remote")).toBe(false);
});

it("rejects a saved removal list that is not a list of ids", () => {
  expect(() => excludedServers(memory("[1]"))).toThrow(
    "The saved UI removal list is invalid.",
  );
});
