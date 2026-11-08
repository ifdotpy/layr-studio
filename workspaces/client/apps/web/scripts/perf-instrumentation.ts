import type { Plugin } from "vite";

// These anchors are deliberately exact: a changed API must not silently omit auth.
function replace(
  source: string,
  anchor: string,
  replacement: string,
  id: string,
) {
  if (source.split(anchor).length !== 2)
    throw new Error(
      `Performance reporting anchor missing or ambiguous in ${id}`,
    );
  return source.replace(anchor, replacement);
}

export function perfInstrumentation(apiOnly = false): Plugin {
  return {
    name: "studio-perf-instrumentation",
    enforce: "pre",
    transform(source, id) {
      if (id.endsWith("/src/api.ts")) {
        source = replace(
          source,
          '  const client = createClient<paths, "application/json">({',
          `  function fetchAuthenticatedRequest(path: string, init: RequestInit) {
    const request = new Request(new URL(path, apiOrigin()), init);
    const headers = new Headers(request.headers);
    headers.set("X-Canvas-Token", token);
    return fetchApiRequest(new Request(request, { headers }));
  }

  const client = createClient<paths, "application/json">({`,
          id,
        );
        source = replace(
          source,
          "    fetchApiRequest,\n    client,",
          "    fetchApiRequest,\n    fetchAuthenticatedRequest,\n    client,",
          id,
        );
        return replace(
          source,
          "  fetchApiRequest,\n  client,",
          "  fetchApiRequest,\n  fetchAuthenticatedRequest,\n  client,",
          id,
        );
      }
      if (apiOnly) return;
      if (id.endsWith("/src/mount.tsx")) {
        source =
          'import { PerformanceReporting } from "./perf/Reporting";\nimport * as serverViewApi from "./api";\n' +
          source;
        return replace(
          source,
          "<ServerServicesProvider services={services}>",
          "<ServerServicesProvider services={services}><PerformanceReporting api={serverViewApi} />",
          id,
        );
      }
      if (id.endsWith("/src/servers/MultiServerApp.tsx")) {
        source =
          'import { PerformanceReporting } from "../perf/Reporting";\n' +
          source;
        return replace(
          source,
          "      {shell}\n    </ServerServicesProvider>",
          "      <PerformanceReporting api={localServices.api} />\n      {shell}\n    </ServerServicesProvider>",
          id,
        );
      }
      return undefined;
    },
  };
}
