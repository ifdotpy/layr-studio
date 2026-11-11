//! Opaque JSON evidence carried without interpreting its contents.

/// Declare a serializable named payload with typed fields and exact raw extensions.
/// Extra owner/field attributes and visibilities remain application-owned.
#[macro_export]
macro_rules! raw_json_payload {
    (flat $(#[$attr:meta])* $visibility:vis struct $owner:ident {
        $($(#[$field_attr:meta])* $field_visibility:vis $member:ident: $ty:ty,)*
        @extensions $(#[$extra_attr:meta])* $extra_visibility:vis $extra:ident $(,)?
    }) => {
        #[derive(serde::Serialize)]
        $(#[$attr])*
        $visibility struct $owner {
            $($(#[$field_attr])* $field_visibility $member: $ty,)*
            #[serde(flatten)]
            $(#[$extra_attr])*
            $extra_visibility $extra: $crate::raw_json::RawExtensions,
        }
        impl<'de> serde::Deserialize<'de> for $owner {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                #[derive(serde::Deserialize)]
                $(#[$attr])*
                struct KnownFields {
                    $($(#[$field_attr])* $member: $ty,)*
                }
                let (known, extra) = $crate::raw_json::deserialize_with_raw_extensions::<D, KnownFields>(deserializer)?;
                Ok(Self { $($member: known.$member,)* $extra: extra })
            }
        }
    };
    (
        $(#[$owner_attr:meta])* $visibility:vis struct $owner:ident {
            $(#[$fields_attr:meta])* $fields_visibility:vis $fields_member:ident: $fields:ty,
            $(#[$extensions_attr:meta])* $extensions_visibility:vis $extensions_member:ident $(,)?
        }
    ) => {
        #[derive(serde::Serialize)]
        $(#[$owner_attr])*
        $visibility struct $owner {
            #[serde(flatten)]
            $(#[$fields_attr])*
            $fields_visibility $fields_member: $fields,
            #[serde(flatten)]
            $(#[$extensions_attr])*
            $extensions_visibility $extensions_member: $crate::raw_json::RawExtensions,
        }
        impl<'de> serde::Deserialize<'de> for $owner {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let (fields, extensions) = $crate::raw_json::deserialize_with_raw_extensions(deserializer)?;
                Ok(Self { $fields_member: fields, $extensions_member: extensions })
            }
        }
    };
}
#[cfg(test)]
use serde::Deserialize;
pub use studio_json::raw_json::{RawExtensions, RawJson};

/// Decode a struct's own fields and retain unknown members without Serde's flatten buffer.
///
/// Exact stored bytes belong to the enclosing ExactJson codec, not this helper:
/// known fields are decoded and extension keys are sorted when re-encoded.
/// Duplicate unknown keys keep their last raw value.
/// The known-field owner must use derived deserialize_struct, without flatten,
/// transparent wrappers, or a custom Deserialize implementation.
pub fn deserialize_with_raw_extensions<'de, D, T>(
    deserializer: D,
) -> Result<(T, RawExtensions), D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    struct FieldNames<'a>(&'a mut Option<&'static [&'static str]>);
    impl<'de> serde::Deserializer<'de> for FieldNames<'_> {
        type Error = serde::de::value::Error;
        fn deserialize_any<V: serde::de::Visitor<'de>>(
            self,
            _: V,
        ) -> Result<V::Value, Self::Error> {
            Err(serde::de::Error::custom("expected a struct"))
        }
        fn deserialize_struct<V: serde::de::Visitor<'de>>(
            self,
            _: &'static str,
            fields: &'static [&'static str],
            _: V,
        ) -> Result<V::Value, Self::Error> {
            *self.0 = Some(fields);
            Err(serde::de::Error::custom("struct fields captured"))
        }
        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes byte_buf
            option unit unit_struct newtype_struct seq tuple tuple_struct map enum identifier ignored_any
        }
    }
    let mut fields = None;
    let _introspection = T::deserialize(FieldNames(&mut fields));
    let fields = fields
        .ok_or_else(|| serde::de::Error::custom("raw extensions require a named struct owner"))?;
    struct ObjectVisitor<T> {
        fields: &'static [&'static str],
        owner: std::marker::PhantomData<T>,
    }
    impl<'de, T: serde::de::DeserializeOwned> serde::de::Visitor<'de> for ObjectVisitor<T> {
        type Value = (T, RawExtensions);
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an object with typed fields and raw extensions")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let (mut known, mut extra) = (RawExtensions::new(), RawExtensions::new());
            while let Some(key) = map.next_key::<String>()? {
                let value = map.next_value()?;
                if let Some(field) = self.fields.iter().find(|field| **field == key) {
                    if known.insert(key, value).is_some() {
                        return Err(serde::de::Error::duplicate_field(field));
                    }
                } else {
                    extra.insert(key, value);
                }
            }
            let encoded = studio_json::to_string(&known).map_err(serde::de::Error::custom)?;
            let typed = studio_json::from_str(&encoded).map_err(serde::de::Error::custom)?;
            Ok((typed, extra))
        }
    }
    deserializer.deserialize_map(ObjectVisitor {
        fields,
        owner: std::marker::PhantomData,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, Deserialize, PartialEq)]
    struct Fields {
        #[serde(rename = "ownerId", alias = "owner")]
        id: String,
        #[serde(default)]
        enabled: bool,
        #[serde(default)]
        optional: Option<String>,
        #[serde(default)]
        presence: crate::queue_command::Field<String>,
        #[serde(skip)]
        derived: Option<String>,
    }
    #[derive(Debug, PartialEq)]
    struct Evidence(Fields, RawExtensions);
    impl<'de> Deserialize<'de> for Evidence {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            deserialize_with_raw_extensions(deserializer).map(|(fields, extra)| Self(fields, extra))
        }
    }
    #[test]
    fn raw_extensions_keep_unknown_tokens_and_reject_duplicate_known_fields() {
        let raw =
            " \n{\"ownerId\":\"\\u0061\",\"future\": { \"numbers\": [1.00,9007199254740993] }} \t";
        let evidence: Evidence = studio_json::from_str(raw).unwrap();
        assert_eq!(evidence.0.id, "a");
        assert_eq!(
            evidence.1["future"].as_str(),
            r#"{ "numbers": [1.00,9007199254740993] }"#
        );
        assert!(studio_json::from_str::<Evidence>(r#"{"ownerId":"a","ownerId":"b"}"#).is_err());
        assert!(studio_json::from_str::<Evidence>(r#"{"ownerId":4}"#).is_err());
        assert!(studio_json::from_str::<Evidence>(r#"{}"#).is_err());
        let duplicate: Evidence =
            studio_json::from_str(r#"{"ownerId":"a","future":1,"future":2.00}"#).unwrap();
        assert_eq!(duplicate.1["future"].as_str(), "2.00");
    }

    #[test]
    fn raw_extensions_preserve_derived_field_defaults_presence_and_alias_rules() {
        let missing: Evidence =
            studio_json::from_str(r#"{"ownerId":"a","derived":{"opaque":true}}"#).unwrap();
        assert!(!missing.0.enabled);
        assert_eq!(missing.0.optional, None);
        assert_eq!(missing.0.derived, None);
        assert!(matches!(
            missing.0.presence,
            crate::queue_command::Field::Missing
        ));
        assert_eq!(missing.1["derived"].as_str(), r#"{"opaque":true}"#);
        let null: Evidence =
            studio_json::from_str(r#"{"owner":"a","optional":null,"presence":null}"#).unwrap();
        assert_eq!(null.0.id, "a");
        assert_eq!(null.0.optional, None);
        assert!(matches!(null.0.presence, crate::queue_command::Field::Null));
        let present: Evidence = studio_json::from_str(
            r#"{"ownerId":"a","enabled":true,"optional":"x","presence":"y"}"#,
        )
        .unwrap();
        assert!(present.0.enabled);
        assert_eq!(present.0.optional.as_deref(), Some("x"));
        assert!(
            matches!(present.0.presence, crate::queue_command::Field::Value(ref value) if value == "y")
        );
        assert!(studio_json::from_str::<Evidence>(r#"{"ownerId":"a","owner":"b"}"#).is_err());
        for raw in ["null", "[]", "true", "7", r#""text""#] {
            assert!(studio_json::from_str::<Evidence>(raw).is_err(), "{raw}");
        }
    }

    #[test]
    fn raw_extensions_reencode_in_key_order_without_rewriting_tokens() {
        let evidence: Evidence =
            studio_json::from_str(r#"{"z":2.00,"ownerId":"a","a": {"x":[1, 2]}}"#).unwrap();
        assert_eq!(
            studio_json::to_string(&evidence.1).unwrap(),
            r#"{"a":{"x":[1, 2]},"z":2.00}"#
        );
    }
}

#[cfg(test)]
mod publication_owner_tests {
    use crate::generated;

    #[test]
    fn publication_view_maps_preserve_parsed_values() {
        fn check<T: serde::de::DeserializeOwned + serde::Serialize>() {
            let raw = r#"{"future":1,"future": { "n":1.00,"n":9007199254740993,"s":"\u0061" }}"#;
            let value: T = studio_json::from_str(raw).unwrap();
            assert_eq!(
                studio_json::to_value(value).unwrap(),
                studio_json::from_str::<studio_json::Value>(raw).unwrap()
            );
        }
        check::<generated::ProviderObject>();
        check::<generated::AccountCostDataModelBreakdown>();
        check::<generated::CostBreakdownModels>();
        check::<generated::CostBreakdownProviders>();
        let raw = r#"{ "number":1 }"#;
        let arbitrary: generated::JsonValue = studio_json::from_str(raw).unwrap();
        assert_eq!(
            studio_json::to_value(arbitrary).unwrap(),
            studio_json::json!({"number":1.0})
        );
    }

    #[test]
    fn publication_request_and_answer_views_decode_through_generated_unions() {
        let request: generated::Request = studio_json::from_str(
            r#"{"kind":"elicitation","schema":{"type":"object","future":true}}"#,
        )
        .unwrap();
        assert_eq!(
            studio_json::to_value(request).unwrap()["schema"]["future"],
            true
        );
        let tool: generated::Request = studio_json::from_str(
            r#"{"kind":"tool_use","tool":"example","input":{"future":[1,null]}}"#,
        )
        .unwrap();
        assert_eq!(
            studio_json::to_value(tool).unwrap()["input"]["future"],
            studio_json::json!([1, null])
        );
        let answer: generated::Answer = studio_json::from_str(
            r#"{"kind":"elicitation","action":"accept","content":{"future":true}}"#,
        )
        .unwrap();
        assert_eq!(
            studio_json::to_value(answer).unwrap()["content"]["future"],
            true
        );
    }

    #[test]
    fn publication_payload_provider_fields_preserve_raw_content() {
        let request: generated::RequestVariant4 =
            studio_json::from_str(r#"{"kind":"elicitation","schema": { "x":1.00,"x":2 }}"#)
                .unwrap();
        assert_eq!(request.schema.0.as_str(), r#"{ "x":1.00,"x":2 }"#);
        let tool: generated::RequestVariant6 = studio_json::from_str(
            r#"{"kind":"tool_use","tool":"example","input": [1.00,9007199254740993]}"#,
        )
        .unwrap();
        assert_eq!(tool.input.0.as_str(), "[1.00,9007199254740993]");
        let answer: generated::AnswerVariant3 = studio_json::from_str(
            r#"{"kind":"elicitation","action":"accept","content": { "x":1.00,"x":2 }}"#,
        )
        .unwrap();
        assert_eq!(answer.content.0.as_str(), r#"{ "x":1.00,"x":2 }"#);
    }
}
