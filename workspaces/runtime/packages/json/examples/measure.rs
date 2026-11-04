use serde::Deserialize;
use std::{collections::BTreeMap, hint::black_box};
use studio_json::Value;

#[path = "../fixtures/fixtures.rs"]
mod fixtures;
use fixtures::FIXTURES;

#[derive(Deserialize)]
struct Record {
    id: Option<String>,
    status: Option<String>,
    name: Option<String>,
    #[serde(flatten)]
    rest: BTreeMap<String, Value>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name = args.get(1).expect("fixture");
    let operation = args.get(2).expect("operation");
    let fixture = FIXTURES
        .iter()
        .find(|(n, _)| n == name)
        .expect("known fixture")
        .1;
    let value: Value = serde_json::from_slice(fixture).expect("fixture JSON");
    match operation.as_str() {
        #[cfg(feature = "serde-backend")]
        "raw-direct" => {
            for _ in 0..100 {
                black_box(
                    serde_json::from_slice::<&serde_json::value::RawValue>(black_box(fixture))
                        .expect("raw"),
                );
            }
        }
        #[cfg(feature = "serde-backend")]
        "raw-facade" => {
            for _ in 0..100 {
                black_box(
                    studio_json::from_slice::<studio_json::RawJson<'_>>(black_box(fixture))
                        .expect("raw"),
                );
            }
        }
        "value" => {
            for _ in 0..100 {
                black_box(
                    studio_json::from_slice_owned::<Value>(black_box(fixture)).expect("parse"),
                );
            }
        }
        "direct" => {
            for _ in 0..100 {
                black_box(serde_json::from_slice::<Value>(black_box(fixture)).expect("parse"));
            }
        }
        "typed" => {
            for _ in 0..100 {
                let record = studio_json::from_slice_owned::<Record>(black_box(fixture))
                    .expect("typed parse");
                black_box((record.id, record.status, record.name, record.rest));
            }
        }
        "serialize" => {
            for _ in 0..100 {
                black_box(studio_json::to_vec(black_box(&value)).expect("serialize"));
            }
        }
        _ => panic!("unknown operation"),
    }
}
