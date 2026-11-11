import type { GetResult } from "../../../api.ts";

type Configuration = GetResult<"/api/orchestration/settings">;
export type { Configuration };

type Settings = Configuration["settings"];

/** One whole-number orchestration setting and its generated bounds. */
export interface OrchestrationSettingSpec {
  field: Exclude<keyof Settings, "idleCompaction" | "shellPolicy">;
  label: string;
  help: string;
  "aria-label": string;
  title: string;
  min: number;
  max: number;
}
