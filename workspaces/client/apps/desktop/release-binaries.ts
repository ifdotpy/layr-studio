// Owner of the Rust release artifacts that both desktop packagers ship, and of the
// commit and SHA-256 manifest that lets a packager or a rehearsal reuse binaries
// built once for the exact same commit instead of building them again.
//
//   node release-binaries.ts build [directory]   build for HEAD, write the manifest
//   node release-binaries.ts verify [directory]  check the manifest against HEAD
//
// The directory is a Cargo target directory. Its `release/` folder holds the
// binaries and the manifest. With no argument the directory is
// CODEX_RELEASE_TARGET_DIR, else `<cache>/codex-studio/release-target`.
import type { ExecFileSyncOptions } from "node:child_process";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { runtimeBinaries, runtimeBinaryName } from "./runtime-binaries.ts";

// Environment variable that selects the prebuilt release target directory. Unit:
// absolute or repository-relative directory path.
export const releaseTargetEnvironment = "CODEX_RELEASE_TARGET_DIR";
// File name of the manifest inside `<directory>/release/`, next to the binaries.
export const releaseManifestName = "studio-release-manifest.json";
export const releaseManifestVersion = 1;

// Cargo packages whose release artifacts the packagers copy.
export const releaseCargoPackages = Object.freeze([
  ...runtimeBinaries.map((entry) => entry.package),
  "studio-diagnostics",
  "studio-peer-link",
]);

// File names below `<directory>/release/` that the manifest must list.
export function releaseArtifacts(platform = process.platform) {
  return [
    ...runtimeBinaries.map(({ binary }) => runtimeBinaryName(binary, platform)),
    "codex-diagnostics",
    "studio-peer-link",
  ];
}

export function cargoReleaseArguments() {
  return [
    "build",
    "--locked",
    "--release",
    ...releaseCargoPackages.flatMap((name) => ["-p", name]),
  ];
}

type Run = (
  command: string,
  args: string[],
  options: ExecFileSyncOptions,
) => unknown;

/** Names of the crates built from this checkout (path sources, not registry). */
export function localCrateNames(repository: string): string[] {
  const metadata: { packages: { name: string; source: string | null }[] } =
    JSON.parse(
      execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], {
        cwd: repository,
        encoding: "utf8",
        maxBuffer: 256 * 1024 * 1024,
      }),
    );
  return [
    ...new Set(
      metadata.packages
        .filter((entry) => entry.source === null)
        .map((entry) => entry.name),
    ),
  ];
}

/**
 * Cargo names a local crate by its path inside the workspace and judges it
 * fresh by file times, and a layr line keeps the file times of its store. A
 * reused target directory can then hold another checkout's crate that looks
 * fresh. Registry crates are fixed by Cargo.lock and stay; this removes the
 * release artifacts of the local crates, so the next build compiles them from
 * this checkout. Without `directory`, Cargo uses the checkout's own target.
 */
export function cleanLocalCrates({
  repository,
  directory,
  run = (command, args, options) =>
    execFileSync(command, args, { stdio: "inherit", ...options }),
  names = () => localCrateNames(repository),
}: {
  repository: string;
  directory?: string | undefined;
  run?: Run;
  names?: () => string[];
}) {
  run(
    "cargo",
    ["clean", "--release", ...names().flatMap((name) => ["-p", name])],
    {
      cwd: repository,
      env: {
        ...process.env,
        ...(directory ? { CARGO_TARGET_DIR: directory } : {}),
      },
    },
  );
}

/** How often a release build waiting for the target lock checks it, in ms. */
const TARGET_LOCK_POLL_MS = 1000;
const alive = (pid: number) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
};

/**
 * One release build at a time in a target directory: another checkout's build
 * between removing the local crates and building would leave its crates fresh
 * again. A lock whose process has ended is taken over.
 */
export async function withTargetLock<T>(
  directory: string,
  body: () => Promise<T>,
): Promise<T> {
  const lock = `${directory}.lock`;
  await mkdir(path.dirname(lock), { recursive: true });
  for (;;) {
    try {
      await mkdir(lock);
      break;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
    }
    const owner = Number(
      await readFile(path.join(lock, "pid"), "utf8").catch(() => "0"),
    );
    if (owner && !alive(owner)) {
      await rm(lock, { recursive: true, force: true });
      continue;
    }
    await new Promise((resolve) => setTimeout(resolve, TARGET_LOCK_POLL_MS));
  }
  await writeFile(path.join(lock, "pid"), String(process.pid));
  try {
    return await body();
  } finally {
    await rm(lock, { recursive: true, force: true });
  }
}

export function releaseTargetDirectory(env = process.env) {
  if (env[releaseTargetEnvironment])
    return path.resolve(env[releaseTargetEnvironment]);
  const cache = env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache");
  return path.join(cache, "codex-studio", "release-target");
}

export function manifestPath(directory: string) {
  return path.join(directory, "release", releaseManifestName);
}

export function sha256File(file: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    createReadStream(file)
      .on("error", reject)
      .on("data", (chunk) => hash.update(chunk))
      .on("end", () => resolve(hash.digest("hex")));
  });
}

// The commit the checkout is on, and whether tracked files differ from it. A
// dirty tree builds different binaries than its commit names.
export function checkoutState(repository: string) {
  const git = (...args: string[]) =>
    execFileSync("git", args, { cwd: repository, encoding: "utf8" }).trim();
  return {
    commit: git("rev-parse", "HEAD"),
    dirty: git("status", "--porcelain", "--untracked-files=no") !== "",
  };
}

export type ReleaseManifest = Awaited<ReturnType<typeof writeReleaseManifest>>;
export type ReleaseIdentity = {
  directory: string;
  commit: string;
  platform?: NodeJS.Platform;
  arch?: string;
};
export async function writeReleaseManifest({
  directory,
  commit,
  platform = process.platform,
  arch = process.arch,
}: ReleaseIdentity) {
  const files: Record<string, string> = {};
  for (const name of releaseArtifacts(platform)) {
    const file = path.join(directory, "release", name);
    files[name] = await sha256File(file);
  }
  const manifest = {
    version: releaseManifestVersion,
    commit,
    platform,
    arch,
    files,
  };
  await writeFile(
    manifestPath(directory),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  return manifest;
}

// Throws unless the manifest records exactly `commit` and every listed binary
// still has its recorded SHA-256. Returns the manifest.
export async function verifyReleaseBinaries({
  directory,
  commit,
  platform = process.platform,
  arch = process.arch,
}: ReleaseIdentity) {
  const where = `Prebuilt release binaries in ${directory}`;
  let manifest: ReleaseManifest;
  try {
    // This program writes the release manifest; existing identity and hash checks follow.
    manifest = JSON.parse(await readFile(manifestPath(directory), "utf8"));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT")
      throw new Error(
        `${where} have no manifest. Run \`just build-release-binaries\`.`,
        { cause: error },
      );
    throw new Error(
      `${where} have an unreadable manifest: ${(error as Error).message}`,
      { cause: error },
    );
  }
  if (manifest?.version !== releaseManifestVersion)
    throw new Error(`${where} have an unsupported manifest version.`);
  if (!/^[a-f0-9]{40}$/.test(commit ?? ""))
    throw new Error("The expected commit must be a 40-digit hexadecimal hash.");
  if (manifest.commit !== commit)
    throw new Error(
      `${where} were built from commit ${manifest.commit}, not ${commit}. Run \`just build-release-binaries\`.`,
    );
  if (manifest.platform !== platform || manifest.arch !== arch)
    throw new Error(
      `${where} were built for ${manifest.platform}/${manifest.arch}, not ${platform}/${arch}.`,
    );
  const listed = Object.keys(manifest.files ?? {}).sort();
  const required = [...releaseArtifacts(platform)].sort();
  if (JSON.stringify(listed) !== JSON.stringify(required))
    throw new Error(
      `${where} list ${listed.join(", ")}; the packager needs ${required.join(", ")}.`,
    );
  for (const name of required) {
    let actual;
    try {
      actual = await sha256File(path.join(directory, "release", name));
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT")
        throw new Error(`${where} are missing ${name}.`, { cause: error });
      throw error;
    }
    if (actual !== manifest.files[name])
      throw new Error(
        `${where} changed after the build: SHA-256 of ${name} differs from the manifest.`,
      );
  }
  return manifest;
}

// Build the release artifacts once for the current commit. The previous manifest
// is removed first, so an interrupted build never leaves a manifest that matches.
export async function buildReleaseBinaries({
  repository,
  directory,
  run = (command, args, options) =>
    execFileSync(command, args, { stdio: "inherit", ...options }),
  state = () => checkoutState(repository),
  localCrates = () => localCrateNames(repository),
}: {
  repository: string;
  directory: string;
  run?: Run;
  state?: () => ReturnType<typeof checkoutState>;
  localCrates?: () => string[];
}) {
  const before = state();
  if (before.dirty)
    throw new Error(
      "Tracked files differ from HEAD. Commit or discard them so the binaries match one commit.",
    );
  return withTargetLock(directory, async () => {
    await mkdir(path.join(directory, "release"), { recursive: true });
    await rm(manifestPath(directory), { force: true });
    cleanLocalCrates({ repository, directory, run, names: localCrates });
    run("cargo", cargoReleaseArguments(), {
      cwd: repository,
      env: { ...process.env, CARGO_TARGET_DIR: directory },
    });
    const after = state();
    if (after.dirty || after.commit !== before.commit)
      throw new Error(
        "The checkout changed during the build; no manifest was written.",
      );
    return writeReleaseManifest({ directory, commit: before.commit });
  });
}

// For a packager. Returns null when no prebuilt directory is selected, so the
// default flow stays a normal Cargo build. Otherwise returns the verified
// directory, or throws for binaries of another commit or a dirty checkout.
export async function prebuiltReleaseDirectory(
  repository: string,
  env = process.env,
  state = () => checkoutState(repository),
) {
  if (!env[releaseTargetEnvironment]) return null;
  const directory = path.resolve(repository, env[releaseTargetEnvironment]);
  const { commit, dirty } = state();
  if (dirty)
    throw new Error(
      "Tracked files differ from HEAD; prebuilt release binaries cannot match this checkout.",
    );
  await verifyReleaseBinaries({ directory, commit });
  return directory;
}

async function main(argv: string[]) {
  const [command, argument] = argv;
  const repository = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../../..",
  );
  const directory = argument
    ? path.resolve(argument)
    : releaseTargetDirectory();
  if (command === "build") {
    const manifest = await buildReleaseBinaries({ repository, directory });
    console.log(`${directory} commit ${manifest.commit}`);
  } else if (command === "verify") {
    const { commit } = checkoutState(repository);
    const manifest = await verifyReleaseBinaries({ directory, commit });
    console.log(`${directory} commit ${manifest.commit} verified`);
  } else {
    throw new Error("Usage: release-binaries.ts build|verify [directory]");
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main(process.argv.slice(2)).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
