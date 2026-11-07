import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  utimes,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  buildReleaseBinaries,
  cleanLocalCrates,
  manifestPath,
  prebuiltReleaseDirectory,
  releaseArtifacts,
  releaseTargetEnvironment,
  verifyReleaseBinaries,
  withTargetLock,
  writeReleaseManifest,
} from "../../../../client/apps/desktop/release-binaries.ts";
import {
  parseFlowArguments,
  planFlow,
} from "../../../../client/apps/native/package-flow.ts";

const commitA = "a".repeat(40);
const commitB = "b".repeat(40);

async function fakeRelease(commit = commitA) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "release-binaries-"));
  await mkdir(path.join(directory, "release"));
  for (const name of releaseArtifacts())
    await writeFile(path.join(directory, "release", name), `binary ${name}`);
  await writeReleaseManifest({ directory, commit });
  return directory;
}

async function withRelease(
  callback: (directory: string) => Promise<void>,
  commit?: string,
) {
  const directory = await fakeRelease(commit);
  try {
    await callback(directory);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

test("accepts binaries built from the same commit", () =>
  withRelease(async (directory) => {
    const manifest = await verifyReleaseBinaries({
      directory,
      commit: commitA,
    });
    assert.equal(manifest.commit, commitA);
    assert.deepEqual(
      Object.keys(manifest.files).sort(),
      [...releaseArtifacts()].sort(),
    );
  }));

test("names the shipped release artifacts for each Unix platform", () => {
  assert.deepEqual(releaseArtifacts("darwin"), [
    "studio-runtime-server",
    "codex-process-supervisor",
    "codex-diagnostics",
    "studio-peer-link",
  ]);
  assert.deepEqual(releaseArtifacts("linux"), [
    "studio-runtime-server",
    "codex-process-supervisor",
    "codex-diagnostics",
    "studio-peer-link",
  ]);
});

test("refuses binaries built from another commit", () =>
  withRelease(async (directory) => {
    await assert.rejects(
      verifyReleaseBinaries({ directory, commit: commitB }),
      new RegExp(`built from commit ${commitA}, not ${commitB}`),
    );
  }));

test("refuses a binary that changed after the build", () =>
  withRelease(async (directory) => {
    const [artifact] = releaseArtifacts();
    assert.ok(artifact);
    await writeFile(
      path.join(directory, "release", artifact),
      "rebuilt elsewhere",
    );
    await assert.rejects(
      verifyReleaseBinaries({ directory, commit: commitA }),
      /SHA-256 of .* differs from the manifest/,
    );
  }));

test("refuses another platform, a missing binary and a missing manifest", () =>
  withRelease(async (directory) => {
    await assert.rejects(
      verifyReleaseBinaries({
        directory,
        commit: commitA,
        arch: "other",
      }),
      /were built for/,
    );
    const [, artifact] = releaseArtifacts();
    assert.ok(artifact);
    await rm(path.join(directory, "release", artifact));
    await assert.rejects(
      verifyReleaseBinaries({ directory, commit: commitA }),
      /are missing/,
    );
    await rm(manifestPath(directory));
    await assert.rejects(
      verifyReleaseBinaries({ directory, commit: commitA }),
      /have no manifest/,
    );
  }));

test("refuses a manifest that omits a required artifact", () =>
  withRelease(async (directory) => {
    const manifest: Awaited<ReturnType<typeof writeReleaseManifest>> =
      JSON.parse(await readFile(manifestPath(directory), "utf8"));
    const [artifact] = releaseArtifacts();
    assert.ok(artifact);
    delete manifest.files[artifact];
    await writeFile(manifestPath(directory), JSON.stringify(manifest));
    await assert.rejects(
      verifyReleaseBinaries({ directory, commit: commitA }),
      /the packager needs/,
    );
  }));

test("a packager uses no prebuilt directory unless one is selected", async () => {
  assert.equal(
    await prebuiltReleaseDirectory("/repo", {}, () => assert.fail()),
    null,
  );
});

test("a packager refuses another commit and a dirty checkout", () =>
  withRelease(async (directory) => {
    const env = { [releaseTargetEnvironment]: directory };
    assert.equal(
      await prebuiltReleaseDirectory("/repo", env, () => ({
        commit: commitA,
        dirty: false,
      })),
      directory,
    );
    await assert.rejects(
      prebuiltReleaseDirectory("/repo", env, () => ({
        commit: commitB,
        dirty: false,
      })),
      /built from commit/,
    );
    await assert.rejects(
      prebuiltReleaseDirectory("/repo", env, () => ({
        commit: commitA,
        dirty: true,
      })),
      /differ from HEAD/,
    );
  }));

test("a build removes the old manifest first and records the built commit", async () => {
  const directory = await fakeRelease(commitB);
  try {
    const calls: {
      command: string;
      args: string[];
      target: string | undefined;
    }[] = [];
    const manifest = await buildReleaseBinaries({
      repository: "/repo",
      directory,
      state: () => ({ commit: commitA, dirty: false }),
      localCrates: () => ["studio-runtime-server", "studio-json"],
      run: (command, args, options) => {
        calls.push({ command, args, target: options.env!.CARGO_TARGET_DIR });
        return verifyReleaseBinaries({ directory, commit: commitB }).then(
          () => assert.fail("the old manifest must be gone during the build"),
          (error: unknown) =>
            assert.match((error as Error).message, /have no manifest/),
        );
      },
    });
    assert.equal(manifest.commit, commitA);
    // The local crates are removed first, in the same target directory.
    const [clean, call] = calls;
    assert.deepEqual(clean, {
      command: "cargo",
      args: [
        "clean",
        "--release",
        "-p",
        "studio-runtime-server",
        "-p",
        "studio-json",
      ],
      target: directory,
    });
    assert.ok(call);
    assert.equal(call.command, "cargo");
    assert.ok(call.args.includes("--locked"));
    assert.ok(call.args.includes("--release"));
    assert.equal(call.target, directory);
    await verifyReleaseBinaries({ directory, commit: commitA });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("a build refuses a dirty checkout and a commit that moved", async () => {
  const directory = await fakeRelease();
  try {
    await assert.rejects(
      buildReleaseBinaries({
        repository: "/repo",
        directory,
        state: () => ({ commit: commitA, dirty: true }),
        run: () => assert.fail("no build"),
      }),
      /differ from HEAD/,
    );
    const states = [
      { commit: commitA, dirty: false },
      { commit: commitB, dirty: false },
    ];
    await assert.rejects(
      buildReleaseBinaries({
        repository: "/repo",
        directory,
        state: () => states.shift()!,
        localCrates: () => [],
        run: () => {},
      }),
      /changed during the build/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("the package flow runs the dev-profile host check unless CI is stated", () => {
  const { out, hostedCiPassed } = parseFlowArguments(["/tmp/out"]);
  assert.equal(hostedCiPassed, false);
  const plan = (skip: boolean) =>
    planFlow({ out, hostedCiPassed: skip, directory: "/r", commit: commitA });
  assert.deepEqual(
    plan(false).map((step) => step.label),
    [
      "signing check",
      "native host check",
      "release binaries",
      "package",
      "check package",
      "check package start",
    ],
  );
  const [signing, native] = plan(false);
  const signingScript = signing?.args?.[0];
  assert.ok(signingScript);
  assert.match(signingScript, /desktop\/signing-check\.ts$/);
  assert.equal(native?.args?.[0], "test-native-webview");
  const [, skipped] = plan(true);
  assert.ok(skipped);
  assert.equal(skipped.command, undefined);
  assert.match(skipped!.skipped!, new RegExp(commitA));
  assert.equal(plan(true).length, 6);
  assert.equal(
    plan(false).some(
      (step) =>
        (step as typeof step & { resolvePython?: boolean }).resolvePython ||
        (step as typeof step & { needsPython?: boolean }).needsPython ||
        step.command === "python3",
    ),
    false,
  );
  const packageStart = plan(false).at(-1)?.args?.[0];
  assert.ok(packageStart);
  assert.match(packageStart, /check-package-start\.ts$/);
  assert.equal(
    parseFlowArguments(["/tmp/out", "--hosted-ci-passed"]).hostedCiPassed,
    true,
  );
  assert.throws(
    () => parseFlowArguments(["/tmp/out", "--skip"]),
    /Unknown flag/,
  );
  assert.throws(() => parseFlowArguments([]), /Usage/);
});

// A layr line keeps the file times of its store, and Cargo names a local crate
// by its path inside the workspace. A target directory reused by two checkouts
// then holds the other checkout's crate as fresh, unless a release build removes
// the local crates first.
test("a reused target directory rebuilds local crates from this checkout", async () => {
  const work = await mkdtemp(path.join(os.tmpdir(), "release-local-crates-"));
  const target = path.join(work, "target");
  const now = Date.now() / 1000;
  const checkout = async (name: string, value: string, seconds: number) => {
    const root = path.join(work, name);
    await mkdir(path.join(root, "src"), { recursive: true });
    const files = {
      "Cargo.toml":
        '[package]\nname = "witness"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n',
      "src/main.rs": `fn main() { print!("${value}"); }\n`,
    };
    for (const [file, text] of Object.entries(files)) {
      await writeFile(path.join(root, file), text);
      await utimes(path.join(root, file), seconds, seconds);
    }
    return root;
  };
  const build = (root: string) => {
    execFileSync("cargo", ["build", "--release", "--quiet"], {
      cwd: root,
      env: { ...process.env, CARGO_TARGET_DIR: target },
      stdio: "inherit",
    });
    return execFileSync(path.join(target, "release", "witness"), {
      encoding: "utf8",
    });
  };
  try {
    assert.equal(build(await checkout("a", "A", now - 1000)), "A");
    // This checkout's files are older than the other checkout's build.
    const b = await checkout("b", "B", now - 2000);
    assert.equal(build(b), "A", "Cargo reuses the other checkout's crate");
    cleanLocalCrates({ repository: b, directory: target });
    assert.equal(build(b), "B");
  } finally {
    await rm(work, { recursive: true, force: true });
  }
});

test("release builds in one target directory run one at a time", async () => {
  const work = await mkdtemp(path.join(os.tmpdir(), "release-lock-"));
  const directory = path.join(work, "target");
  const events: string[] = [];
  try {
    let release!: () => void;
    const first = withTargetLock(directory, async () => {
      events.push("first start");
      await new Promise<void>((resolve) => (release = resolve));
      events.push("first end");
    });
    while (!release) await new Promise((resolve) => setTimeout(resolve, 10));
    const second = withTargetLock(directory, async () => {
      events.push("second");
    });
    await new Promise((resolve) => setTimeout(resolve, 50));
    release();
    await Promise.all([first, second]);
    assert.deepEqual(events, ["first start", "first end", "second"]);
    // A lock left by an ended process is taken over.
    await mkdir(`${directory}.lock`);
    await writeFile(path.join(`${directory}.lock`, "pid"), "999999");
    assert.equal(await withTargetLock(directory, async () => "taken"), "taken");
  } finally {
    await rm(work, { recursive: true, force: true });
  }
});
