//! Single-pass object-only decoding for HTTP request contracts.

use std::{fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};

/// Decode a typed value only from a JSON object, never from a positional array.
pub struct Object<T>(pub T);

/// Field adapter for nested object-only values.
pub fn deserialize_object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    Object::deserialize(deserializer).map(|value| value.0)
}

/// Field adapter for an optional object where explicit null is invalid.
pub fn deserialize_optional_object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    deserialize_object(deserializer).map(Some)
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object")
            }
            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(Object)
            }
        }
        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Nested {
        value: u64,
    }
    #[derive(Deserialize)]
    struct Outer {
        #[serde(default, deserialize_with = "super::deserialize_optional_object")]
        nested: Option<Nested>,
    }

    #[test]
    fn optional_object_rejects_null_and_arrays() {
        assert!(
            serde_json::from_str::<Outer>("{}")
                .expect("missing")
                .nested
                .is_none()
        );
        assert_eq!(
            serde_json::from_str::<Outer>(r#"{"nested":{"value":3}}"#)
                .expect("object")
                .nested
                .expect("nested")
                .value,
            3
        );
        assert!(serde_json::from_str::<Outer>(r#"{"nested":null}"#).is_err());
        assert!(serde_json::from_str::<Outer>(r#"{"nested":[3]}"#).is_err());
    }
}
