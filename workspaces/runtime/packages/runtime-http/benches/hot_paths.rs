use std::hint::black_box;

use axum::response::sse::Event;
use gungraun::{Dhat, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use serde_json::{Value, json};
use studio_runtime_http::routes::sse_event;

fn payload() -> Value {
    json!({"protocol":1,"workspaceId":"workspace","epoch":"epoch","revision":42,"resources":[{"kind":"agent","id":"worker"}],"resourceVersions":[{"revision":42,"items":[{"id":"item","text":"a transcript line"}]}]})
}

#[library_benchmark]
#[bench::resource(payload())]
fn event(payload: Value) -> Event {
    sse_event("resources", Some(42), black_box(payload))
}

library_benchmark_group!(name = http, benchmarks = [event]);
main!(
    config = LibraryBenchmarkConfig::default().tool(Dhat::default()),
    library_benchmark_groups = http
);
