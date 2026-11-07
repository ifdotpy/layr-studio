import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";

/** Compile the actual question fixture and serve only its generated assets. */
export async function createBuiltQuestionServer(
  entry: string,
  options: { port?: number } = {},
) {
  const scratch = await mkdtemp(join(tmpdir(), "built-question-"));
  const root = fileURLToPath(new URL("../../", import.meta.url));
  const input = join(scratch, "index.html"),
    outDir = join(scratch, "dist");
  await writeFile(
    input,
    '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><div id="root"></div><script type="module" src="/fixture.tsx"></script>',
  );
  try {
    await build({
      configFile: false,
      root: scratch,
      resolve: {
        alias: { "/fixture.tsx": join(root, "tests/questions", entry) },
      },
      logLevel: "error",
      build: { outDir, emptyOutDir: true, rollupOptions: { input } },
    });
  } catch (error) {
    await rm(scratch, { recursive: true, force: true });
    throw error;
  }
  const types: Record<string, string> = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".css": "text/css",
    ".wasm": "application/wasm",
  };
  const server = createServer(async (request, response) => {
    const path = new URL(request.url || "/", "http://fixture").pathname;
    const file = path.startsWith("/assets/") ? path.slice(1) : "index.html";
    try {
      response.setHeader(
        "Content-Type",
        types[extname(file)] || "application/octet-stream",
      );
      response.end(await readFile(join(outDir, file)));
    } catch {
      response.statusCode = 404;
      response.end();
    }
  });
  return {
    resolvedUrls: { local: [""] },
    async listen() {
      await new Promise<void>((resolve) =>
        server.listen(options.port || 0, "127.0.0.1", resolve),
      );
      const address = server.address();
      if (!address || typeof address === "string")
        throw new Error("Missing fixture address");
      this.resolvedUrls.local[0] = `http://127.0.0.1:${address.port}/`;
    },
    async close() {
      await new Promise<void>((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      );
      await rm(scratch, { recursive: true, force: true });
    },
  };
}
