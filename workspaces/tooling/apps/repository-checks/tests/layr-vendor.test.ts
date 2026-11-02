import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

// The vendored layr tree must hold exactly the files that its manifest.json pins to
// the layr revision, with the same SHA-256 hashes. Covers src/ and crates/.
const vendoredDirectories = ["src", "crates"];

function listFiles(root: string, directory: string): string[] {
  const full = path.join(root, directory);
  if (!existsSync(full)) return [];
  return readdirSync(full, { withFileTypes: true }).flatMap((entry) => {
    const relative = `${directory}/${entry.name}`;
    return entry.isDirectory() ? listFiles(root, relative) : [relative];
  });
}

function sha256(file: string): string {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

// Returns one line per difference. An empty list means the tree matches the manifest.
function vendorDrift(layrRoot: string): string[] {
  const manifest: { files: Record<string, string> } = JSON.parse(
    readFileSync(path.join(layrRoot, "manifest.json"), "utf8"),
  );
  const pinned = new Map(
    Object.entries(manifest.files).filter(([file]) =>
      vendoredDirectories.some((directory) => file.startsWith(`${directory}/`)),
    ),
  );
  const onDisk = new Set(
    vendoredDirectories.flatMap((directory) => listFiles(layrRoot, directory)),
  );
  const drift: string[] = [];
  for (const file of onDisk) {
    if (!pinned.has(file)) drift.push(`not in manifest: ${file}`);
  }
  for (const [file, hash] of pinned) {
    if (!onDisk.has(file)) drift.push(`missing: ${file}`);
    else if (sha256(path.join(layrRoot, file)) !== hash) {
      drift.push(`changed: ${file}`);
    }
  }
  return drift;
}

test("the vendored layr tree matches the files pinned by its manifest", () => {
  const layrRoot = fileURLToPath(
    new URL("../../../../runtime/apps/layr/", import.meta.url),
  );
  assert.deepEqual(vendorDrift(layrRoot), []);
});

test("the vendor check reports an added, removed or changed file", () => {
  const root = mkdtempSync(path.join(tmpdir(), "layr-vendor-"));
  try {
    mkdirSync(path.join(root, "src"));
    const file = path.join(root, "src", "a.rs");
    const extra = path.join(root, "src", "extra.rs");
    writeFileSync(file, "a");
    writeFileSync(
      path.join(root, "manifest.json"),
      JSON.stringify({
        files: { "src/a.rs": createHash("sha256").update("a").digest("hex") },
      }),
    );
    assert.deepEqual(vendorDrift(root), []);
    writeFileSync(extra, "x");
    assert.deepEqual(vendorDrift(root), ["not in manifest: src/extra.rs"]);
    rmSync(extra);
    writeFileSync(file, "changed");
    assert.deepEqual(vendorDrift(root), ["changed: src/a.rs"]);
    rmSync(file);
    assert.deepEqual(vendorDrift(root), ["missing: src/a.rs"]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
