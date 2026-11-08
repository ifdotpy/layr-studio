/** Every sync page fixture creates its mount element before loading its module. */
export function fixtureElement(id: string) {
  return document.getElementById(id)!;
}

export type FixtureSnapshot = ReturnType<
  typeof import("./startup-fixture.ts").useStartupFixture
>;

/** Probes inspect legacy payloads from the fixture's resource event document. */
export type FixtureResourceFrame = Omit<
  import("../playwright.ts").ResourceChangeEvent,
  "resourceVersions"
> & {
  resourceVersions: (Omit<
    import("../playwright.ts").ResourceChangeEvent["resourceVersions"][number],
    "entityChanges"
  > & {
    entityChanges?: Omit<
      import("../../src/generated/api.ts").components["schemas"]["EntityChangeBatch"],
      "documents"
    > & {
      documents: import("./entity-fixture-protocol.ts").FixtureEntity[];
    };
  })[];
};
