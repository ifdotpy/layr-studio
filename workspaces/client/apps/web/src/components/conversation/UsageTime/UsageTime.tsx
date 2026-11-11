import { localDateTime } from "../../../local-time.ts";

export function UsageTime({ at }: { at: number }) {
  const date = new Date(at * 1000);
  return (
    <time dateTime={date.toISOString()} title={date.toString()}>
      {localDateTime(date, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      })}
    </time>
  );
}
