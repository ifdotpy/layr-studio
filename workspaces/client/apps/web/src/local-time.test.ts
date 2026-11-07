import { describe, expect, it } from "vitest";
import { localDateTime, localTime } from "./local-time";

// The cached formatters must print exactly what the Date methods print.
describe("cached local time formatters", () => {
  const dates = [
    new Date(2026, 10, 6, 0, 5, 9),
    new Date(2026, 0, 31, 23, 59, 1),
  ];
  const options: Intl.DateTimeFormatOptions[] = [
    {},
    { hour: "2-digit", minute: "2-digit" },
    { hour: "2-digit", minute: "2-digit", second: "2-digit" },
    { month: "short", day: "numeric" },
    { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" },
    {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    },
    { weekday: "short" },
    { dateStyle: "medium" },
    { dateStyle: "medium", timeStyle: "short" },
    { timeStyle: "short" },
  ];
  it.each(options)("matches toLocaleString for %j", (option) => {
    for (const date of dates)
      expect(localDateTime(date, option)).toBe(
        date.toLocaleString(undefined, { ...option, hourCycle: "h23" }),
      );
  });
  it.each(options.filter((option) => !option.dateStyle))(
    "matches toLocaleTimeString for %j",
    (option) => {
      for (const date of dates)
        expect(localTime(date, option)).toBe(
          date.toLocaleTimeString(undefined, { ...option, hourCycle: "h23" }),
        );
    },
  );
});
