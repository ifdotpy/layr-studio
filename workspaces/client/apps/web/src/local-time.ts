type LocalTimeOptions = Omit<
  Intl.DateTimeFormatOptions,
  "timeZone" | "hour12" | "hourCycle"
>;

const dateFields = ["weekday", "year", "month", "day"] as const;
const timeFields = [
  "dayPeriod",
  "hour",
  "minute",
  "second",
  "fractionalSecondDigits",
] as const;
const numericDate: LocalTimeOptions = {
  year: "numeric",
  month: "numeric",
  day: "numeric",
};
const numericTime: LocalTimeOptions = {
  hour: "numeric",
  minute: "numeric",
  second: "numeric",
};
/** How often the device time zone is read again, in milliseconds. */
const ZONE_CHECK_MS = 1000;
const formats = new Map<string, Intl.DateTimeFormat>();
let zone = "";
let zoneCheckedAt = -Infinity;

/**
 * A cached formatter: building one per message row cost most of a streamed
 * reply's render time. A device time zone change still applies within
 * ZONE_CHECK_MS, as the cache is cleared when the zone differs.
 */
function formatter(options: Intl.DateTimeFormatOptions) {
  const now = Date.now();
  if (now - zoneCheckedAt > ZONE_CHECK_MS) {
    zoneCheckedAt = now;
    const current = new Intl.DateTimeFormat().resolvedOptions().timeZone;
    if (current !== zone) {
      zone = current;
      formats.clear();
    }
  }
  const key = JSON.stringify(options);
  let format = formats.get(key);
  if (!format) {
    format = new Intl.DateTimeFormat(undefined, {
      ...options,
      hourCycle: "h23",
    });
    formats.set(key, format);
  }
  return format;
}

const has = (options: LocalTimeOptions, fields: readonly string[]) =>
  fields.some(
    (field) => field in options && options[field as never] !== undefined,
  );

/** The same text as Date#toLocaleString with these options and h23. */
export function localDateTime(date: Date, options: LocalTimeOptions = {}) {
  const styled = !!(options.dateStyle || options.timeStyle);
  const bare =
    !styled && !has(options, dateFields) && !has(options, timeFields);
  return formatter(
    bare ? { ...numericDate, ...numericTime, ...options } : options,
  ).format(date);
}

/** The same text as Date#toLocaleTimeString with these options and h23. */
export function localTime(date: Date, options: LocalTimeOptions = {}) {
  const timed = !!options.timeStyle || has(options, timeFields);
  return formatter(timed ? options : { ...options, ...numericTime }).format(
    date,
  );
}

// Provider notices contain text clocks, rather than timestamp fields.
export function serviceTimeText(text: string, reference = new Date()): string {
  if (!/^(?:You've hit|You’ve hit|Claude usage limit reached)/i.test(text))
    return text;
  let result = text.replace(
    /\b(\d{1,2})(?::([0-5]\d))?\s*([ap])m\s*\(([A-Za-z_]+(?:\/[A-Za-z_+-]+)+)\)/gi,
    (original, hour, minute, period, zone) => {
      if (+hour < 1 || +hour > 12 || !Number.isFinite(reference.getTime()))
        return original;
      try {
        const source = new Intl.DateTimeFormat("en-US", {
          timeZone: zone,
          year: "numeric",
          month: "2-digit",
          day: "2-digit",
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
          hourCycle: "h23",
        });
        const fields = (date: Date) =>
          new Map(
            source.formatToParts(date).map((part) => [part.type, part.value]),
          );
        const day = fields(reference);
        const clock = Date.UTC(
          Number(day.get("year")),
          Number(day.get("month")) - 1,
          Number(day.get("day")),
          (+hour % 12) + (period.toLowerCase() === "p" ? 12 : 0),
          +(minute || 0),
        );
        let instant = clock;
        for (let n = 0; n < 3; n++) {
          const part = fields(new Date(instant));
          const shown = Date.UTC(
            Number(part.get("year")),
            Number(part.get("month")) - 1,
            Number(part.get("day")),
            Number(part.get("hour")),
            Number(part.get("minute")),
            Number(part.get("second")),
          );
          instant += clock - shown;
        }
        return localTime(new Date(instant), {
          hour: "2-digit",
          minute: "2-digit",
        });
      } catch {
        return original;
      }
    },
  );
  result = result.replace(
    /\b\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z\b/g,
    (iso) => localDateTime(new Date(iso)),
  );
  return result.replace(
    /\b(\d{1,2})(?::([0-5]\d))?\s*([ap])m\b/gi,
    (original, hour, minute, period) =>
      +hour >= 1 && +hour <= 12
        ? `${String((+hour % 12) + (period.toLowerCase() === "p" ? 12 : 0)).padStart(2, "0")}:${minute || "00"}`
        : original,
  );
}
