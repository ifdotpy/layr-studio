use std::hint::black_box;

use gungraun::{Dhat, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use serde_json::{Value, json};
use studio_runtime_store::AgentRecord;
use studio_runtime_store::stored_text;

fn text() -> String {
    "{\"aggregatedOutput\":\"line\\n\"}".repeat(4000)
}

fn warm_encode() -> String {
    let input = text();
    black_box(stored_text::encode(&input));
    input
}

fn warm_decode() -> Vec<u8> {
    let input = stored_text::encode(&text());
    black_box(stored_text::decode_blob(&input).expect("valid frame"));
    input
}

#[library_benchmark]
#[bench::transcript(warm_encode())]
fn encode(input: String) -> Vec<u8> {
    stored_text::encode(black_box(&input))
}

#[library_benchmark]
#[bench::transcript(warm_decode())]
fn decode(input: Vec<u8>) -> String {
    stored_text::decode_blob(black_box(&input)).expect("valid frame")
}

fn record() -> Value {
    json!({"compactions":0,"accountKey":"default","provider":"codex","id":"worker","rootId":"lead","epoch":2,"status":"running","autoWake":true,"inFlight":true,"agentMode":"multi","concurrency":8})
}

#[library_benchmark]
#[bench::agent(record().to_string())]
fn record_decode(input: String) -> AgentRecord {
    serde_json::from_str(black_box(&input)).expect("agent record")
}

#[library_benchmark]
#[bench::agent(AgentRecord::from_value(record()).expect("agent record"))]
fn record_encode(input: AgentRecord) -> String {
    serde_json::to_string(black_box(&input)).expect("agent record")
}

library_benchmark_group!(
    name = store,
    benchmarks = [encode, decode, record_decode, record_encode]
);
main!(
    config = LibraryBenchmarkConfig::default().tool(Dhat::default()),
    library_benchmark_groups = store
);
