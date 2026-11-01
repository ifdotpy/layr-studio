//! Typed known fields of provider request parameters and preview.
use crate::queue_command::Field;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Command {
    Text(String),
    Arguments(Vec<String>),
}

macro_rules! fields {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, Default, Deserialize, Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            $(#[serde(default, skip_serializing_if = "Field::is_missing")]
            pub $field: Field<$ty>,)*
        }
    };
}

pub type Object = BTreeMap<String, Box<RawValue>>;
fields!(RequestParams {
    changes: Box<RawValue>, client_id: String, command: Command, cwd: String,
    expires_at: f64, host: String, linux_user: String, message: String,
    paste: Object, permissions: Object, phase: String, questions: Vec<Question>,
    reason: String, redirect: Object, requested_schema: RequestSchema, scope: String,
    source: String, thread_id: String, turn_id: String, url: String, user_code: String,
    waiting: Object,
});
fields!(RequestPreview {
    changes: Box<RawValue>, command: Command, cwd: String, permissions: Object, url: String,
});
#[derive(Clone, Serialize)]
pub struct QuestionOption {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub description: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub label: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub value: Field<Box<RawValue>>,
    #[serde(flatten)]
    pub extra: Object,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub id: String,
    pub question: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub is_secret: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub multi_select: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub options: Field<Vec<QuestionOption>>,
    #[serde(flatten)]
    pub extra: Object,
}

impl<'de> Deserialize<'de> for QuestionOption {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = QuestionOption;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a question option")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut result = QuestionOption {
                    description: Field::Missing,
                    label: Field::Missing,
                    value: Field::Missing,
                    extra: Object::new(),
                };
                let mut seen = std::collections::BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if matches!(key.as_str(), "description" | "label" | "value")
                        && !seen.insert(key.clone())
                    {
                        return Err(serde::de::Error::custom("duplicate question option field"));
                    }
                    match key.as_str() {
                        "description" => result.description = map.next_value()?,
                        "label" => result.label = map.next_value()?,
                        "value" => result.value = map.next_value()?,
                        _ => {
                            result.extra.insert(key, map.next_value()?);
                        }
                    }
                }
                Ok(result)
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
impl<'de> Deserialize<'de> for Question {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Question;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a question")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let (mut id, mut question) = (None, None);
                let (mut is_secret, mut multi_select, mut options) =
                    (Field::Missing, Field::Missing, Field::Missing);
                let mut extra = Object::new();
                let mut seen = std::collections::BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if matches!(
                        key.as_str(),
                        "id" | "question" | "isSecret" | "multiSelect" | "options"
                    ) && !seen.insert(key.clone())
                    {
                        return Err(serde::de::Error::custom("duplicate question field"));
                    }
                    match key.as_str() {
                        "id" => id = Some(map.next_value()?),
                        "question" => question = Some(map.next_value()?),
                        "isSecret" => is_secret = map.next_value()?,
                        "multiSelect" => multi_select = map.next_value()?,
                        "options" => options = map.next_value()?,
                        _ => {
                            extra.insert(key, map.next_value()?);
                        }
                    }
                }
                Ok(Question {
                    id: id.ok_or_else(|| serde::de::Error::missing_field("id"))?,
                    question: question
                        .ok_or_else(|| serde::de::Error::missing_field("question"))?,
                    is_secret,
                    multi_select,
                    options,
                    extra,
                })
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
#[derive(Clone, Deserialize, Serialize)]
pub struct RequestSchema {
    pub properties: BTreeMap<String, RequestSchemaProperty>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub required: Field<Vec<String>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSchemaProperty {
    #[serde(rename = "enum", default, skip_serializing_if = "Field::is_missing")]
    pub variants: Field<Vec<Box<RawValue>>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub format: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub is_secret: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub items: Field<Box<RequestSchemaProperty>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub title: Field<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Field::is_missing")]
    pub kind: Field<Command>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub write_only: Field<bool>,
    #[serde(flatten)]
    pub extra: Object,
}
impl<'de> Deserialize<'de> for RequestSchemaProperty {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = RequestSchemaProperty;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a request schema property")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut result = RequestSchemaProperty {
                    variants: Field::Missing,
                    format: Field::Missing,
                    is_secret: Field::Missing,
                    items: Field::Missing,
                    title: Field::Missing,
                    kind: Field::Missing,
                    write_only: Field::Missing,
                    extra: Object::new(),
                };
                let mut seen = std::collections::BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if matches!(
                        key.as_str(),
                        "enum" | "format" | "isSecret" | "items" | "title" | "type" | "writeOnly"
                    ) && !seen.insert(key.clone())
                    {
                        return Err(serde::de::Error::custom("duplicate request schema field"));
                    }
                    match key.as_str() {
                        "enum" => result.variants = map.next_value()?,
                        "format" => result.format = map.next_value()?,
                        "isSecret" => result.is_secret = map.next_value()?,
                        "items" => result.items = map.next_value()?,
                        "title" => result.title = map.next_value()?,
                        "type" => result.kind = map.next_value()?,
                        "writeOnly" => result.write_only = map.next_value()?,
                        _ => {
                            result.extra.insert(key, map.next_value()?);
                        }
                    }
                }
                Ok(result)
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
