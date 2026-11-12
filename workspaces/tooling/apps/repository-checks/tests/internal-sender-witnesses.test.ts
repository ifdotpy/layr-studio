import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  discoverSenderSites,
  senderWitnessErrors,
} from "../internal-sender-witnesses.ts";

test("every production provider sender registers an existing real-codec witness", () => {
  const files = execFileSync("git", ["ls-files", "-z", "workspaces/runtime"], {
    encoding: "utf8",
  }).split("\0");
  const sites = discoverSenderSites(files);
  assert.ok(sites.length > 20, "discover the production boundaries");
  assert.deepEqual(senderWitnessErrors(files), []);
  assert.ok(
    readFileSync(
      "workspaces/tooling/apps/repository-checks/facade.ts",
      "utf8",
    ).includes("tests/internal-sender-witnesses.test.ts"),
  );
});

test("sender discovery uses expressions and reports missing or stale witnesses at file:line", () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "sender-witness-"));
  try {
    const file = "sender.rs";
    const source = `
fn production() {
    let request = ProviderRequest { method, params };
    // internal-sender-witness: sender.rs#internal_sender_witness_missing
    control.prepare_frame(operation, frame);
    // internal-sender-witness: sender.rs#internal_sender_witness_present
    let typed = codex::ClientRequest::ConfigRead { id, params };
    let nested = vec![ProviderRequest { method, params }];
    let error = MonitorError::ProviderRequest { stage, source };
    let text = "Frame::Request(request)";
    // control.operation_status(operation);
    match frame { Frame::Request(request) => (), _ => () }
}
#[cfg(test)] mod tests { fn ignored() { ProviderRequest { method, params }; } }
#[test] fn internal_sender_witness_present() {}
`;
    writeFileSync(path.join(directory, file), source);
    assert.deepEqual(
      discoverSenderSites([file], directory).map((site) => site.line),
      [3, 5, 7, 8],
    );
    assert.deepEqual(senderWitnessErrors([file], directory), [
      "sender.rs:3: missing internal-sender-witness annotation",
      "sender.rs:5: witness does not exist: sender.rs#internal_sender_witness_missing",
      "sender.rs:8: missing internal-sender-witness annotation",
    ]);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("both parsed fast filters select the sender witnesses", () => {
  const config = readFileSync(".config/nextest.toml", "utf8");
  const sections = config.split(/(?=^\[)/m);
  const filters = sections.flatMap((section) => {
    if (!/^\[\[?profile\.fast(?:\.overrides)?\]\]?/.test(section)) return [];
    const filter = section.match(/default-filter\s*=\s*'''([\s\S]*?)'''/);
    return filter ? [filter[1] ?? ""] : [];
  });
  assert.equal(
    filters.length,
    2,
    "parse the common and Linux override filters",
  );
  assert.deepEqual(
    filters.map((filter) => filter.includes("test(internal_sender_witness_)")),
    [true, true],
  );
});

function discoveryFixture(source: string): number[] {
  const directory = mkdtempSync(path.join(os.tmpdir(), "sender-witness-"));
  try {
    writeFileSync(path.join(directory, "sender.rs"), source);
    return discoverSenderSites(["sender.rs"], directory).map(
      (site) => site.line,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("real alias idiom and provider send boundaries cannot bypass discovery", () => {
  assert.deepEqual(
    discoveryFixture(`
use crate::account_session_runtime::{ProviderRequest as SessionRequest, ProviderResponse};
fn send() {
    session.provider_request(SessionRequest { method, params });
    session.transfer_provider_request(request);
    let request = SessionRequest { method, params };
    session.provider_request(request);
    AccountSessionPort::provider_request(session, request);
}
`),
    [4, 5, 7, 8],
  );
});

test("nested production modules remain visible beside test children", () => {
  assert.deepEqual(
    discoveryFixture(`
mod owner {
    fn send() { ProviderRequest { method, params }; }
    #[cfg(test)] #[allow(dead_code)] mod checks { fn hidden() { ProviderRequest { method, params }; } }
    mod tests { fn hidden() { ProviderRequest { method, params }; } }
}
`),
    [3],
  );
});

test("macro requests preserve qualification and exclude error variants", () => {
  assert.deepEqual(
    discoveryFixture(`
fn send() {
    let actual = vec![ProviderRequest { method, params }];
    let qualified = vec![session::ProviderRequest { method, params }];
    let errors = vec![MonitorError::ProviderRequest { stage, source }];
}
`),
    [3, 4],
  );
});

test("retained session request and macro boundaries cover alias/helper payloads", () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "sender-witness-"));
  try {
    const file = "account_session_runtime.rs";
    writeFileSync(
      path.join(directory, file),
      `
use crate::account_session_runtime::ProviderRequest as SessionRequest;
fn alias_direct() { session.request(SessionRequest { method, params }); }
fn make_request() -> SessionRequest { SessionRequest { method, params } }
fn helper_direct() { session.request(make_request()); }
fn macro_send() { tokio::join!(session.provider_request(ExportedRequest { method, params })); }
fn macro_transfer() { tokio::join!(session.transfer_provider_request(make_request())); }
fn macro_session() { tokio::join!(session.request(make_request())); }
fn unrelated() { client.request(make_request()); tokio::join!(client.request(make_request())); }
`,
    );
    assert.deepEqual(
      discoverSenderSites([file], directory).map((site) => site.line),
      [3, 5, 6, 7, 8],
    );
    writeFileSync(
      path.join(directory, "unrelated.rs"),
      `
fn unrelated() { session.request(make_request()); tokio::join!(session.request(make_request())); }
`,
    );
    assert.deepEqual(
      discoverSenderSites(["unrelated.rs"], directory),
      [],
      "plain request is scoped to the retained account-session implementation",
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
