import { sidebarRowFixture } from "../../src/servers/sidebarRowFixture.ts";

export const peerTeamRowFixtures = {
  a: sidebarRowFixture("a", { name: "Alpha", projectPath: "/p", sortKey: "0" }),
  b: sidebarRowFixture("b", {
    name: "Beta",
    projectPath: "/p",
    projectFolder: "f",
    sortKey: "1",
  }),
  c: sidebarRowFixture("c", { name: "Gamma", projectPath: "/p", sortKey: "2" }),
  x: sidebarRowFixture("x", {
    name: "Other project",
    projectPath: "/other",
    sortKey: "3",
  }),
  e: sidebarRowFixture("e", {
    name: "Epsilon",
    projectPath: "/p",
    sortKey: "4",
  }),
};
