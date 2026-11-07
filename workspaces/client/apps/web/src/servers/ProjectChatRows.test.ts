import { expect, it } from "vitest";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import { sidebarFactsFixture } from "./sidebarSourceFixture.ts";
import { required } from "../components/componentTestFixtures.ts";
import {
  compactVisibleChats,
  keepCompactChat,
  shouldShowOldChatsControl,
} from "./ProjectChatRows";
function chat(id: string, updated = 1, pinned = false) {
  const source = sidebarFactsFixture([
    agentSummaryFixture(id, { updated, pinned }),
  ]);
  return { kind: "chat" as const, id, row: required(source.view.rows[0]) };
}
it("uses shared view compact policy and the activity window", () => {
  const now = 200000;
  expect(keepCompactChat(chat("old"), now)).toBe(false);
  expect(keepCompactChat(chat("pin", 1, true), now)).toBe(true);
  expect(keepCompactChat(chat("recent", now - 86399), now)).toBe(true);
  expect(keepCompactChat(chat("boundary", now - 86400), now)).toBe(false);
});
it("applies old-chat threshold to the entire project", () => {
  const now = Date.now() / 1000,
    recent = chat("recent", now),
    recentTwo = chat("recent-two", now),
    old = chat("old");
  const two = [recent, old],
    three = [recent, recentTwo, old];
  expect(shouldShowOldChatsControl(two.length, 3)).toBe(false);
  expect(compactVisibleChats(two, true, 3)).toEqual(two);
  expect(compactVisibleChats(three, true, 3)).toEqual([recent, recentTwo]);
  expect(compactVisibleChats(three, false, 3)).toEqual(three);
});
