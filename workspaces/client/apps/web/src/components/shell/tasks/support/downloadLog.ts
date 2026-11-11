const DOWNLOAD_URL_REVOKE_MS = 10_000;
/** Starts a download and releases its object URL after the browser consumes it. */
export function downloadLog({ blob, name }: { blob: Blob; name: string }) {
  const url = URL.createObjectURL(blob),
    link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), DOWNLOAD_URL_REVOKE_MS);
}
