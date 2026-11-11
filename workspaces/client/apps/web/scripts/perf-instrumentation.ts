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

export function perfInstrumentation(): Plugin {
  return {
    name: "studio-perf-instrumentation",
    enforce: "pre",
    transform(source, id) {
      if (id.endsWith("/src/servers/net/api.ts")) {
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
      return undefined;
    },
  };
}
