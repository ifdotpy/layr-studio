export function panelWidthAfterResize(
  width: number,
  startX: number,
  currentX: number,
  min: number,
  max: number,
): number | undefined {
  if (
    ![width, startX, currentX, min, max].every(Number.isFinite) ||
    min <= 0 ||
    max < min
  )
    return undefined;
  return Math.min(max, Math.max(min, width + startX - currentX));
}
