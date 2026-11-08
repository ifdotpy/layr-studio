export type MessageDateProps = { at?: number | string | null | undefined };

// The device time zone is captured here; reload after changing it.
// The device locale is also captured at load; changing it takes effect after reload.
const formatter = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});

export default function MessageDate({ at }: MessageDateProps) {
  const value =
    at == null ||
    at === "" ||
    (typeof at === "number" && (!Number.isFinite(at) || at <= 0))
      ? null
      : new Date(typeof at === "number" ? at * 1000 : at);
  const valid = value && Number.isFinite(value.getTime());
  return (
    <time
      className="message-date"
      dateTime={valid ? value.toISOString() : undefined}
    >
      {valid ? formatter.format(value) : "Date unavailable"}
    </time>
  );
}
