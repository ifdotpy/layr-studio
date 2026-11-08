export type WindowAnchor = {
  id: string;
  offset: number;
  kind?: "message" | "row";
  paragraph?: number;
  fallbackOffset?: number;
};

export const anchors = new Map<string, WindowAnchor | null>();

export const followsStoredAnchor = (key?: string) => !key || !anchors.get(key);
