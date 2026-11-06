use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use studio_json::{Value, from_slice, from_slice_owned, from_str, from_str_owned};

#[cfg(feature = "serde-backend")]
#[path = "../fixtures/fixtures.rs"]
mod fixtures;

#[cfg(feature = "serde-backend")]
#[test]
fn every_fixture_serializes_to_identical_bytes() {
    for (name, input) in fixtures::FIXTURES {
        let value: Value = serde_json::from_slice(input).expect("fixture");
        assert_eq!(
            studio_json::to_vec(&value).expect("facade"),
            serde_json::to_vec(&value).expect("reference"),
            "{name}"
        );
        assert_eq!(
            studio_json::to_string(&value).expect("facade"),
            serde_json::to_string(&value).expect("reference"),
            "{name}"
        );
    }
}

#[cfg(feature = "simd-backend")]
#[test]
fn nested_owned_parse_panics_while_scratch_is_borrowed() {
    struct Nested;
    impl<'de> Deserialize<'de> for Nested {
        fn deserialize<D: serde::Deserializer<'de>>(_: D) -> std::result::Result<Self, D::Error> {
            let _: Value = from_str_owned("null").map_err(serde::de::Error::custom)?;
            Ok(Self)
        }
    }
    assert!(std::panic::catch_unwind(|| from_str_owned::<Nested>("null")).is_err());
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    name_field: String,
    #[serde(default)]
    count: u64,
    #[serde(flatten)]
    kind: Kind,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Kind {
    Text { text: String },
    Number { value: i64 },
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(untagged)]
enum Untagged {
    Number(i64),
    Text(String),
}

#[test]
fn typed_round_trip_and_attributes() {
    let text = r#"{"nameField":"demo","type":"text","text":"line\n\uD83D\uDE00"}"#;
    let expected: Record = serde_json::from_str(text).expect("fixture");
    assert_eq!(from_str::<Record>(text).expect("typed"), expected);
    assert_eq!(from_str_owned::<Record>(text).expect("owned"), expected);
    let encoded = studio_json::to_string(&expected).expect("encode");
    assert_eq!(
        serde_json::from_str::<Record>(&encoded).expect("round trip"),
        expected
    );
    assert_eq!(
        from_str::<Untagged>("17").expect("untagged"),
        Untagged::Number(17)
    );
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Strict {
        count: u64,
    }
    let strict: Strict = from_str(r#"{"count":1}"#).expect("strict");
    assert_eq!(strict.count, 1);
    assert!(from_str::<Strict>(r#"{"count":1,"extra":1}"#).is_err());
}

#[test]
fn borrowed_values() {
    #[derive(Deserialize)]
    struct Borrowed<'a> {
        s: &'a str,
        #[serde(borrow)]
        c: Cow<'a, str>,
    }
    let input = r#"{"s":"hello","c":"world"}"#;
    let parsed: Borrowed<'_> = from_str(input).expect("borrowed");
    assert_eq!(parsed.s, "hello");
    assert!(matches!(parsed.c, Cow::Borrowed("world")));
}

#[test]
fn syntax_and_number_parity() {
    for input in [
        "",
        "null true",
        "[1,]",
        "\"\\uD800\"",
        "18446744073709551616",
        "-9223372036854775809",
        "1e300",
        "-0.0",
        "1.2345678901234567",
        r#"{"x":1,"x":2}"#,
    ] {
        let expected = serde_json::from_str::<Value>(input);
        let actual = from_str::<Value>(input);
        assert_eq!(
            actual.as_ref().ok(),
            expected.as_ref().ok(),
            "borrowed {input}"
        );
        let actual = from_str_owned::<Value>(input);
        assert_eq!(
            actual.as_ref().ok(),
            expected.as_ref().ok(),
            "owned {input}"
        );
    }
    for bytes in [b"\xff".as_slice(), b"\"\xff\"".as_slice()] {
        assert!(from_slice::<Value>(bytes).is_err());
        assert!(from_slice_owned::<Value>(bytes).is_err());
    }
}

#[test]
fn integer_boundaries_match() {
    assert_eq!(
        from_str::<u128>("340282366920938463463374607431768211455").ok(),
        serde_json::from_str::<u128>("340282366920938463463374607431768211455").ok()
    );
    assert!(from_str::<u128>("340282366920938463463374607431768211456").is_err());
    for input in ["18446744073709551615", "18446744073709551616", "-1"] {
        assert_eq!(
            from_str::<u64>(input).ok(),
            serde_json::from_str::<u64>(input).ok()
        );
    }
    for input in [
        "9223372036854775807",
        "9223372036854775808",
        "-9223372036854775808",
        "-9223372036854775809",
    ] {
        assert_eq!(
            from_str::<i64>(input).ok(),
            serde_json::from_str::<i64>(input).ok()
        );
    }
}

#[test]
fn negative_zero_keeps_its_sign() {
    let negative_zero = from_str::<f64>("-0.0").expect("negative zero");
    assert!(negative_zero.is_sign_negative());
}

#[test]
fn nesting_limit_matches() {
    let deep = format!("{}0{}", "[".repeat(128), "]".repeat(128));
    assert_eq!(
        from_str::<Value>(&deep).is_ok(),
        serde_json::from_str::<Value>(&deep).is_ok()
    );
}

#[test]
fn raw_values_preserve_the_original_json() {
    #[derive(Debug, Deserialize, Serialize)]
    struct Raw {
        data: Box<studio_json::value::RawValue>,
    }
    let input = r#"{"data":{ "x" : [1,2] }}"#;
    let parsed: Raw = from_str(input).expect("raw value");
    assert_eq!(parsed.data.get(), r#"{ "x" : [1,2] }"#);
    let owned: Raw = from_str_owned(input).expect("owned raw value");
    assert_eq!(owned.data.get(), parsed.data.get());
    assert_eq!(
        studio_json::to_string(&parsed).expect("serialize raw"),
        serde_json::to_string(&parsed).expect("reference")
    );
}

#[test]
fn facade_raw_json_preserves_borrowed_and_owned_record_fields() {
    #[derive(Deserialize, Serialize)]
    struct Record<'a> {
        #[serde(borrow)]
        params: studio_json::RawJson<'a>,
        reply: studio_json::OwnedRawJson,
    }
    let input = r#"{"params":{ "a" : [1,2] },"reply":{ "b" : "x" }}"#;
    let record: Record<'_> = from_str(input).expect("record");
    assert_eq!(record.params.as_str(), r#"{ "a" : [1,2] }"#);
    assert_eq!(record.reply.as_str(), r#"{ "b" : "x" }"#);
    assert_eq!(record.params.to_owned().as_str(), record.params.as_str());
    assert_eq!(studio_json::to_string(&record).expect("encode"), input);
    let mut output = Vec::new();
    studio_json::to_writer(&mut output, &record).expect("write raw record");
    assert_eq!(output, input.as_bytes());
    let raw = studio_json::OwnedRawJson::from_string("[ 1, 2 ]".into()).expect("raw");
    assert_eq!(raw.as_str(), "[ 1, 2 ]");
    let parsed: studio_json::OwnedRawJson =
        studio_json::from_reader("{ \"a\": 1 }".as_bytes()).expect("read raw");
    assert_eq!(parsed.as_str(), "{ \"a\": 1 }");
}

#[test]
fn owned_raw_json_value_conversion() {
    let raw = studio_json::OwnedRawJson::from_string("{\"a\":1}".into()).expect("raw");
    assert_eq!(
        studio_json::to_value(&raw).expect("to value"),
        studio_json::json!({"a":1})
    );
    let value = studio_json::json!({"a":1});
    let parsed: studio_json::OwnedRawJson = studio_json::from_value(value).expect("from value");
    assert_eq!(parsed.as_str(), "{\"a\":1}");
}

#[cfg(feature = "serde-backend")]
#[test]
fn to_raw_value_matches_serde_bytes() {
    for value in [
        serde_json::json!(null),
        serde_json::json!({"escaped": "a\n\"b", "numbers": [-0.0, 12, 1.25]}),
        serde_json::json!([true, {"unicode": "żółć"}]),
    ] {
        let expected = serde_json::value::to_raw_value(&value).expect("serde raw");
        let actual = studio_json::to_raw_value(&value).expect("facade raw");
        assert_eq!(actual.as_str(), expected.get());
    }
}
<<<<<<< head
=======

#[cfg(feature = "serde-backend")]
#[test]
fn serde_raw_marker_matches_backend() {
    use serde::ser::{Error as _, Impossible, Serializer};

    struct Capture;
    macro_rules! unsupported {
        ($($name:ident($($arg:ident: $ty:ty),*)),* $(,)?) => {
            $(fn $name(self, $(_: $ty),*) -> Result<Self::Ok, Self::Error> {
                unreachable!("raw JSON serializes as a struct")
            })*
        };
    }
    impl Serializer for Capture {
        type Ok = ();
        type Error = serde_json::Error;
        type SerializeSeq = Impossible<(), Self::Error>;
        type SerializeTuple = Impossible<(), Self::Error>;
        type SerializeTupleStruct = Impossible<(), Self::Error>;
        type SerializeTupleVariant = Impossible<(), Self::Error>;
        type SerializeMap = Impossible<(), Self::Error>;
        type SerializeStruct = Impossible<(), Self::Error>;
        type SerializeStructVariant = Impossible<(), Self::Error>;

        unsupported! {
            serialize_bool(v: bool), serialize_i8(v: i8), serialize_i16(v: i16),
            serialize_i32(v: i32), serialize_i64(v: i64), serialize_u8(v: u8),
            serialize_u16(v: u16), serialize_u32(v: u32), serialize_u64(v: u64),
            serialize_f32(v: f32), serialize_f64(v: f64), serialize_char(v: char),
            serialize_str(v: &str), serialize_bytes(v: &[u8]), serialize_none(),
            serialize_unit(), serialize_unit_struct(name: &'static str),
            serialize_unit_variant(name: &'static str, index: u32, variant: &'static str)
        }
        fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_newtype_struct<T: Serialize + ?Sized>(
            self,
            _: &'static str,
            _: &T,
        ) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_newtype_variant<T: Serialize + ?Sized>(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: &T,
        ) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            unreachable!()
        }
        fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
            unreachable!()
        }
        fn serialize_tuple_struct(
            self,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleStruct, Self::Error> {
            unreachable!()
        }
        fn serialize_tuple_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleVariant, Self::Error> {
            unreachable!()
        }
        fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            unreachable!()
        }
        fn serialize_struct(
            self,
            name: &'static str,
            _: usize,
        ) -> Result<Self::SerializeStruct, Self::Error> {
            Err(Self::Error::custom(name))
        }
        fn serialize_struct_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeStructVariant, Self::Error> {
            unreachable!()
        }
    }

    let raw = serde_json::value::RawValue::from_string("{}".into()).unwrap();
    assert_eq!(
        raw.serialize(Capture).unwrap_err().to_string(),
        studio_json::RAW_VALUE_TOKEN
    );
}
>>>>>>> agent-8b610cda59ed4865310e15f0285f6e5ba521d8474d0ed359b5e088286474ab31
