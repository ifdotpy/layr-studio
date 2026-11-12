import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
export const root = fileURLToPath(new URL("../../../..", import.meta.url));
interface ProviderManifest {
  name: string;
  studioIntegration: {
    id: string;
    contracts: string;
    update: string;
    drift: string;
    contractTests: string[];
  };
}
export function discoverProviders(directory = root) {
  const base = path.join(directory, "workspaces/integrations/providers");
  return readdirSync(base, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((entry) => {
      const folder = path.join(base, entry.name);
      const manifest: ProviderManifest = JSON.parse(
        readFileSync(path.join(folder, "package.json"), "utf8"),
      );
      return { folder, ...manifest.studioIntegration };
    });
}
