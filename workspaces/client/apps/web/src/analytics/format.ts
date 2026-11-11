import { localDateTime, localTime } from "../local-time.ts";
export const finite = (n: unknown): n is number =>
  typeof n === "number" && Number.isFinite(n);
export const count = (n: unknown) =>
  finite(n) ? n.toLocaleString() : "Unavailable";
export const bytes = (n: unknown) => {
  if (!finite(n)) return "Unavailable";
  if (n < 1024) return `${count(n)} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KiB`;
  return `${(n / 1024 ** 2).toFixed(2)} MiB`;
};
export const duration = (n: unknown) =>
  !finite(n)
    ? "Unavailable"
    : n < 1000
      ? `${Math.round(n)} ms`
      : n < 60000
        ? `${(n / 1000).toFixed(1)} s`
        : `${(n / 60000).toFixed(1)} min`;
export const time = (n: unknown) =>
  finite(n) ? localDateTime(new Date(n * 1000)) : "Unavailable";
export const shortTime = (n: number) =>
  localTime(new Date(n * 1000), {
    hour: "2-digit",
    minute: "2-digit",
  });
