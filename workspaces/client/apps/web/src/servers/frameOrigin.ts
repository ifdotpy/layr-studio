// WebKit assigns registered native schemes a host origin even though the URL
// standard reports "null". Message destinations must match MessageEvent.origin.
export function frameMessageOrigin(value: string): string {
  const url = new URL(value);
  return url.protocol === "studio-view:" || url.protocol === "tauri:"
    ? `${url.protocol}//${url.host}`
    : url.origin;
}
