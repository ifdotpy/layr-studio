import { expect, it } from "vitest";
import {
  compactVisibleChats,
  keepCompactChat,
  shouldShowOldChatsControl,
} from "./ProjectChatRows";

it("uses server compact eligibility and the shared activity window", () => {
  const old = { id: "old", name: "Old", activityAt: 1, compactPinned: false };
  const now = 200000;
  expect(keepCompactChat(old, now)).toBe(false);
  expect(keepCompactChat({ ...old, compactPinned: true }, now)).toBe(true);
  expect(keepCompactChat({ ...old, activityAt: now - 86399 }, now)).toBe(true);
  expect(keepCompactChat({ ...old, activityAt: now - 86400 }, now)).toBe(false);
});

it("applies the old-chat threshold to all chats in a project", () => {
  const now = Date.now() / 1000;
  const recent = { id: "recent", name: "Recent", compactPinned: false, activityAt: now };
  const recentTwo = { id: "recent-two", name: "Recent two", compactPinned: false, activityAt: now };
  const old = { id: "old", name: "Old", compactPinned: false, activityAt: 1 };
  const twoChats = [recent, old];
  expect(shouldShowOldChatsControl(twoChats.length, 3)).toBe(false);
  expect(compactVisibleChats(twoChats, true, 3)).toEqual(twoChats);

  const threeChats = [recent, recentTwo, old];
  expect(shouldShowOldChatsControl(threeChats.length, 3)).toBe(true);
  const visibleAtThree = compactVisibleChats(threeChats, true, 3);
  expect(visibleAtThree).toEqual([recent, recentTwo]);
  expect(threeChats.length - visibleAtThree.length).toBe(1);
  expect(compactVisibleChats(threeChats, true, 4)).toEqual(threeChats);
  expect(compactVisibleChats(threeChats, false, 3)).toEqual(threeChats);
});
