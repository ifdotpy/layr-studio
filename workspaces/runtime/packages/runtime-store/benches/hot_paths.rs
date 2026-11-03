use std::hint::black_box;

use gungraun::{Dhat, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use studio_runtime_store::stored_text;

fn text() -> String {
    "{\"aggregatedOutput\":\"line\\n\"}".repeat(4000)
}

#[library_benchmark]
#[bench::transcript(text())]
fn encode(input: String) -> Vec<u8> {
    stored_text::encode(black_box(&input))
}

#[library_benchmark]
#[bench::transcript(stored_text::encode(&text()))]
fn decode(input: Vec<u8>) -> String {
    stored_text::decode_blob(black_box(&input)).expect("valid frame")
}

library_benchmark_group!(name = store, benchmarks = [encode, decode]);
main!(
    config = LibraryBenchmarkConfig::default().tool(Dhat::default()),
    library_benchmark_groups = store
);
