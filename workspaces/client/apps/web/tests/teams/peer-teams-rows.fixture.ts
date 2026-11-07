import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";

export const peerTeamRowFixtures = {
  a: agentSummaryFixture("a", { name: "Alpha", projectPath: "/p", cwd: "/p" }),
  b: agentSummaryFixture("b", {
    name: "Beta",
    projectPath: "/p",
    cwd: "/p",
    projectFolder: "f",
  }),
  c: agentSummaryFixture("c", { name: "Gamma", projectPath: "/p", cwd: "/p" }),
  x: agentSummaryFixture("x", {
    name: "Other project",
    projectPath: "/other",
    cwd: "/other",
  }),
  e: agentSummaryFixture("e", {
    name: "Epsilon",
    projectPath: "/p",
    cwd: "/p",
  }),
};
