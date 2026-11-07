//! Pure metadata and TypeScript generation for the observable sidebar pilot.
mod emit;
mod shape;

use schemars::JsonSchema;
use studio_json::Value;

pub use emit::render;

/// A model's Rust schema owner and field-level observation metadata.
#[derive(Clone, Debug)]
pub struct ObservableModel {
    pub schema: String,
    pub identity: Option<String>,
    pub opaque: Vec<String>,
    pub ordered_by: Option<String>,
}
impl ObservableModel {
    pub fn of<T: JsonSchema>() -> Self {
        Self {
            schema: T::schema_name().into_owned(),
            identity: None,
            opaque: vec![],
            ordered_by: None,
        }
    }
    pub fn identity(mut self, field: &str) -> Self {
        self.identity = Some(field.into());
        self
    }
    pub fn ordered_by(mut self, field: &str) -> Self {
        self.ordered_by = Some(field.into());
        self
    }
    pub fn opaque(mut self, field: &str) -> Self {
        self.opaque.push(field.into());
        self
    }
}

/// A string resource argument with a distinct generated nominal type.
#[derive(Clone, Debug)]
pub struct ObservableArg {
    pub name: String,
    pub brand: String,
}

/// One ResourceRef kind, facade and Rust-owned response schema.
#[derive(Clone, Debug)]
pub struct ObservableResource {
    pub kind: String,
    pub facade: String,
    pub data: String,
    pub args: Vec<ObservableArg>,
}
impl ObservableResource {
    pub fn of<T: JsonSchema>(kind: &str, facade: &str, args: Vec<ObservableArg>) -> Self {
        Self {
            kind: kind.into(),
            facade: facade.into(),
            data: T::schema_name().into_owned(),
            args,
        }
    }
}

/// Fail closed on unsupported reachable schemas; paths identify the owning schema.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{path}: {kind}")]
pub struct GenerationError {
    pub path: String,
    pub kind: UnsupportedShape,
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UnsupportedShape {
    #[error("missing schema or reference")]
    MissingSchema,
    #[error("unsupported schema shape")]
    Shape,
    #[error("recursive model")]
    Recursive,
    #[error("invalid or colliding identifier")]
    Identifier,
    #[error("metadata names an unknown field")]
    UnknownField,
    #[error("identity must name a required string field")]
    Identity,
    #[error("union requires unique string literal tags")]
    Union,
    #[error("resource metadata disagrees with ResourceRef or SubscriptionDataMap")]
    Resource,
}
fn fail(path: &str, kind: UnsupportedShape) -> GenerationError {
    GenerationError {
        path: path.into(),
        kind,
    }
}
fn quote(value: &str) -> String {
    studio_json::to_string(value).expect("string serializes")
}
fn identifier(value: &str, path: &str) -> Result<(), GenerationError> {
    if value.is_empty()
        || !value
            .chars()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphabetic() || c == '_' || (i > 0 && c.is_ascii_digit()))
        || matches!(
            value,
            "constructor"
                | "apply"
                | "default"
                | "class"
                | "new"
                | "return"
                | "function"
                | "delete"
                | "var"
                | "let"
                | "const"
                | "export"
                | "import"
                | "extends"
                | "super"
                | "this"
                | "null"
                | "true"
                | "false"
        )
    {
        return Err(fail(path, UnsupportedShape::Identifier));
    }
    Ok(())
}
fn schema<'a>(document: &'a Value, name: &str) -> Result<&'a Value, GenerationError> {
    document
        .pointer(&format!("/components/schemas/{name}"))
        .ok_or_else(|| {
            fail(
                &format!("#/components/schemas/{name}"),
                UnsupportedShape::MissingSchema,
            )
        })
}

#[cfg(test)]
mod tests;
