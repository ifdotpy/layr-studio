import { afterEach, expect, it, vi } from "vitest";
import { DISK_FULL_CAUSE, type CapacityRetry } from "../../../capacityRetry.ts";
import type { PostResult } from "../../../api.ts";
import {
  CapacityRetryStatus,
  UsageResumeCause,
  UsageResumeStatus,
} from "../../../generated/api.ts";
import type { Agent } from "../../../types.ts";
import { capacityProjection, resumeProjection } from "./recovery.ts";

afterEach(() => vi.useRealTimers());

it("selects capacity confirmations by identity and revision and keeps countdown timing", () => {
  vi.useFakeTimers();
  vi.setSystemTime(1_000_000);
  const retry: CapacityRetry = {
    id: "retry",
    accountKey: null,
    dueAt: 1001.2,
    epoch: null,
    status: "scheduled",
    threadId: null,
    updatedAt: 1,
  };
  const confirmed: PostResult<"/api/capacity-retry"> = {
    id: "retry",
    status: CapacityRetryStatus.cancelled,
    updatedAt: 2,
  };
  expect(capacityProjection(retry, undefined)).toMatchObject({
    scheduled: true,
    available: true,
    message: "Automatic retry in 2s.",
  });
  expect(capacityProjection(retry, confirmed)).toMatchObject({
    scheduled: false,
    available: true,
    message: "Automatic retry cancelled.",
  });
  expect(
    capacityProjection({ ...retry, updatedAt: 3, claimedAt: 1 }, confirmed),
  ).toMatchObject({ scheduled: true, available: false });
  expect(
    capacityProjection(retry, { ...confirmed, id: "another" }).current,
  ).toBe(retry);
  expect(
    capacityProjection({ ...retry, cause: DISK_FULL_CAUSE }, undefined).message,
  ).toBe("Waiting for free disk space.");
});

it("projects the live resume record and takes a newer confirmation only", () => {
  const resume: Agent["usageResume"] = {
    id: "resume",
    status: "scheduled",
    cause: "auth",
    dueAt: 2000,
    plannedAt: null,
    reason: null,
    updatedAt: 1,
  };
  const confirmed: PostResult<"/api/usage-resume"> = {
    accountKey: "default",
    cause: UsageResumeCause.auth,
    epoch: 1,
    failedAt: 1,
    threadId: "thread",
    turnId: "turn",
    id: "resume",
    status: UsageResumeStatus.cancelled,
    reason: "Automatic resume is off for this chat.",
    updatedAt: 2,
  };
  expect(resumeProjection(resume, undefined, "agent")).toMatchObject({
    auth: true,
    scheduled: true,
    canToggle: true,
    resumeAt: 2000,
  });
  expect(resumeProjection(resume, confirmed, "agent")).toMatchObject({
    scheduled: false,
    canToggle: true,
  });
  expect(
    resumeProjection(
      { ...resume, updatedAt: 3, plannedAt: 3000 },
      confirmed,
      "agent",
    ),
  ).toMatchObject({ scheduled: true, resumeAt: 3000 });
  expect(resumeProjection(resume, undefined).canToggle).toBe(false);
});
