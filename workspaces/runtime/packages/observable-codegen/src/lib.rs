//! Pure metadata and TypeScript generation for the observable sidebar pilot.
mod collections;
mod emit;
mod inventory;
mod metadata;
mod shape;

use schemars::JsonSchema;
use studio_json::Value;

pub use emit::{render, render_fields};
pub use inventory::render_entity_collections;

/// A model's Rust schema owner and field-level observation metadata.
#[derive(Clone, Debug)]
pub struct ObservableModel {
    pub(crate) schema: String,
    pub identity: Option<String>,
    pub optional_identity: bool,
    pub omitted: Vec<String>,
    pub keyed_by: Vec<String>,
    pub opaque: Vec<String>,
    /// When selected, nested fields outside this list remain frozen plain refs.
    pub tracked_nested: Option<Vec<String>>,
    /// The existing local projection exposes declared fields of an open wire record.
    pub declared_fields: bool,
    pub ordered_by: Option<String>,
}
impl ObservableModel {
    pub fn of<T: JsonSchema>() -> Self {
        Self::named(&T::schema_name())
    }
    /// Refer to an existing component whose schema is already owned by the contract document.
    pub fn named(schema: &str) -> Self {
        Self {
            schema: schema.into(),
            identity: None,
            optional_identity: false,
            omitted: vec![],
            keyed_by: vec![],
            opaque: vec![],
            tracked_nested: None,
            declared_fields: false,
            ordered_by: None,
        }
    }
    pub fn identity(mut self, field: &str) -> Self {
        self.identity = Some(field.into());
        self
    }
    /// Use supplied ids; legacy entries without an id retain their array position identity.
    pub fn identity_or_index(mut self, field: &str) -> Self {
        self.identity = Some(field.into());
        self.optional_identity = true;
        self
    }
    /// Exclude an existing presentation field from the declared domain projection.
    pub fn omit(mut self, field: &str) -> Self {
        self.omitted.push(field.into());
        self
    }
    /// Required string paths that jointly identify a tagged union entry.
    pub fn keyed_by(mut self, paths: &[&str]) -> Self {
        self.keyed_by = paths.iter().map(|path| (*path).into()).collect();
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
    pub fn track_nested(mut self, fields: &[&str]) -> Self {
        self.tracked_nested = Some(fields.iter().map(|field| (*field).into()).collect());
        self
    }
    pub fn declared_fields(mut self) -> Self {
        self.declared_fields = true;
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
    pub(crate) data: String,
    pub args: Vec<ObservableArg>,
    pub collections: Vec<ObservableCollection>,
    pub(crate) mapped: bool,
    pub(crate) opaque: bool,
    pub(crate) mirror_only: bool,
}

/// Select one existing tagged payload variant as a keyed readonly collection.
#[derive(Clone, Debug)]
pub struct ObservableCollection {
    pub collection: String,
    pub facade: String,
    pub schema: String,
}
impl ObservableCollection {
    pub fn named(collection: &str, facade: &str, schema: &str) -> Self {
        Self {
            collection: collection.into(),
            facade: facade.into(),
            schema: schema.into(),
        }
    }
    pub fn of<T: JsonSchema>(collection: &str, facade: &str) -> Self {
        Self::named(collection, facade, &T::schema_name())
    }
}
impl ObservableResource {
    /// Project an existing one-shot response without inventing a subscription resource.
    pub fn mirror(schema: &str, facade: &str) -> Self {
        let mut resource = Self::named(facade, facade, schema, vec![]);
        resource.mirror_only = true;
        resource
    }
    /// Read the response shape from SubscriptionDataMap, including array and union roots.
    pub fn mapped(kind: &str, facade: &str, args: Vec<ObservableArg>) -> Self {
        let mut chars = facade.chars();
        let data = format!(
            "{}{}Data",
            chars.next().expect("facade").to_ascii_uppercase(),
            chars.as_str()
        );
        Self {
            kind: kind.into(),
            facade: facade.into(),
            data,
            args,
            collections: vec![],
            mapped: true,
            opaque: false,
            mirror_only: false,
        }
    }
    /// Explicit boundary for an existing open or untagged JSON transport value.
    pub fn opaque(mut self) -> Self {
        self.opaque = true;
        self
    }
    pub fn named(kind: &str, facade: &str, schema: &str, args: Vec<ObservableArg>) -> Self {
        Self {
            kind: kind.into(),
            facade: facade.into(),
            data: schema.into(),
            args,
            collections: vec![],
            mapped: false,
            opaque: false,
            mirror_only: false,
        }
    }
    pub fn of<T: JsonSchema>(kind: &str, facade: &str, args: Vec<ObservableArg>) -> Self {
        Self {
            kind: kind.into(),
            facade: facade.into(),
            data: T::schema_name().into_owned(),
            args,
            collections: vec![],
            mapped: false,
            opaque: false,
            mirror_only: false,
        }
    }
    /// Adapt an existing singleton array resource; unselected variants remain outside this store.
    pub fn collections(
        kind: &str,
        facade: &str,
        payload: &str,
        collections: Vec<ObservableCollection>,
    ) -> Self {
        Self {
            kind: kind.into(),
            facade: facade.into(),
            data: payload.into(),
            args: vec![],
            collections,
            mapped: false,
            opaque: false,
            mirror_only: false,
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
    #[error("open object or map")]
    OpenObject,
    #[error("unsupported schema composition")]
    Composition,
    #[error("tuple array")]
    Tuple,
    #[error("mixed non-null types")]
    MixedTypes,
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
                | "__proto__"
                | "prototype"
                | "interface"
                | "type"
                | "private"
                | "public"
                | "protected"
                | "static"
                | "yield"
                | "await"
                | "implements"
                | "instanceof"
                | "undefined"
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
