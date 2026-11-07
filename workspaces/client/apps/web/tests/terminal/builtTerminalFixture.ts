import { build, createServer, type Plugin } from "vite";
import { join } from "node:path";

/** Terminal witnesses serve compiled assets, including their controlled xterm adapter. */
export async function builtTerminalFixture(
  root: string,
  temporary: string,
  entry: string,
  plugins: Plugin[] = [],
) {
  const outDir = join(temporary, "dist");
  await build({
    configFile: false,
    root,
    plugins,
    logLevel: "warn",
    define: { __STUDIO_PERF_REPORTING__: "false" },
    build: {
      target: "esnext",
      outDir,
      cssCodeSplit: false,
      rollupOptions: {
        input: join(root, "tests/terminal", entry),
        output: {
          entryFileNames: "fixture.js",
          assetFileNames: "[name].[ext]",
        },
      },
    },
  });
  return createServer({
    configFile: false,
    root: outDir,
    cacheDir: join(temporary, "vite"),
    server: { host: "127.0.0.1", port: 0, hmr: false },
  });
}
