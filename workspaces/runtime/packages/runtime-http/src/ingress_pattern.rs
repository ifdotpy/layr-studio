//! Named HTTP ingress identifiers selected by their OpenAPI pattern.
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct BoundedPattern<T, const MIN: usize, const MAX: usize>(T);
impl<T, const MIN: usize, const MAX: usize> BoundedPattern<T, MIN, MAX> {
    pub fn inner(&self) -> &T {
        &self.0
    }
}
impl<'de, T: serde::de::DeserializeOwned, const MIN: usize, const MAX: usize> Deserialize<'de>
    for BoundedPattern<T, MIN, MAX>
{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::value::StringDeserializer;
        let text = String::deserialize(d)?;
        if !(MIN..=MAX).contains(&text.chars().count()) {
            return Err(serde::de::Error::custom("text outside bounds"));
        }
        T::deserialize(StringDeserializer::<D::Error>::new(text)).map(Self)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct BoundedSafeInteger<const MIN: u64, const MAX: u64>(
    studio_runtime_schema::recovery_record::SafeInteger,
);
impl<const MIN: u64, const MAX: u64> BoundedSafeInteger<MIN, MAX> {
    pub fn get(&self) -> i64 {
        self.0.get() as i64
    }
}
impl<'de, const MIN: u64, const MAX: u64> Deserialize<'de> for BoundedSafeInteger<MIN, MAX> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = studio_runtime_schema::recovery_record::SafeInteger::deserialize(d)?;
        if (MIN as f64..=MAX as f64).contains(&value.get()) {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom("integer outside bounds"))
        }
    }
}

macro_rules! pattern_text {
    ($name:ident, $valid:expr) => {
        #[derive(Clone, Debug, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);
        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = &'static str;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                if ($valid)(&value) {
                    Ok(Self(value))
                } else {
                    Err("invalid ingress identifier")
                }
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Self::try_from(String::deserialize(d)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

fn identifier(value: &str, max: usize, extra: &[u8]) -> bool {
    let mut bytes = value.bytes();
    value.len() <= max
        && bytes.next().is_some_and(|b| b.is_ascii_alphanumeric())
        && bytes.all(|b| b.is_ascii_alphanumeric() || extra.contains(&b))
}
pattern_text!(ProjectId, |v: &str| identifier(v, 100, b"_-"));
pattern_text!(NonWhitespace, |v: &str| v.chars().any(|c| {
    // JSON Schema uses ECMA-262 whitespace: unlike Unicode White_Space,
    // it excludes NEXT LINE and includes the byte-order mark.
    !((c.is_whitespace() && c != '\u{85}') || c == '\u{feff}')
}));
pattern_text!(Uuid, |v: &str| {
    v.len() == 36
        && [8, 13, 18, 23].iter().all(|&i| v.as_bytes()[i] == b'-')
        && uuid::Uuid::parse_str(v).is_ok()
});
pattern_text!(VaultRequestId, |v: &str| identifier(v, 128, b":_-"));
pattern_text!(SecretRequestId, |v: &str| identifier(v, 100, b":_-"));
pattern_text!(AwsProfileName, |v: &str| identifier(v, 128, b"_.@+-"));
pattern_text!(SecretName, |v: &str| {
    let mut bytes = v.bytes();
    v.len() <= 64
        && bytes
            .next()
            .is_some_and(|b| b.is_ascii_uppercase() || b == b'_')
        && bytes.all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
});
pattern_text!(Region, |v: &str| {
    let parts: Vec<_> = v.split('-').collect();
    parts.len() >= 3
        && parts[0].len() == 2
        && parts[0].bytes().all(|b| b.is_ascii_lowercase())
        && parts[1..parts.len() - 1]
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase()))
        && parts
            .last()
            .is_some_and(|part| part.len() == 1 && part.as_bytes()[0].is_ascii_digit())
});

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn named_patterns_match_openapi() {
        let document = crate::schema::openapi_document();
        for (schema, field, baseline) in [
            ("ProjectSecretsListRequest", "projectId", "project"),
            ("ProjectSecretSetRequest", "name", "TOKEN"),
            ("ProjectSecretSetRequest", "requestId", "request"),
            ("VaultLoginRequest", "requestId", "request"),
            ("VaultLoginRequest", "profile", "profile"),
            ("VaultSetupRequest", "prefix", "prefix"),
            ("VaultSetupRequest", "region", "us-east-1"),
            ("AccountGroupRequest", "group", "team_1"),
            ("RenameServer", "label", "Server Name"),
            (
                "ClaudeCancelRequest",
                "login_id",
                "01234567-89ab-cdef-0123-456789abcdef",
            ),
            ("ClaudeAddStartRequest", "label", "label"),
        ] {
            let spec = document["components"]["schemas"][schema]["properties"][field].clone();
            let validator = jsonschema::options()
                .should_validate_formats(true)
                .with_pattern_options(jsonschema::PatternOptions::regex())
                .build(&spec)
                .unwrap();
            let mut values = vec![
                baseline.to_owned(),
                String::new(),
                " ".into(),
                "é".into(),
                "\u{85}".into(),
                "\u{a0}".into(),
                "\u{200b}".into(),
                "\u{2028}".into(),
                "\u{feff}".into(),
                "a\n".into(),
                "x".repeat(44),
                "x".repeat(100),
                "x".repeat(128),
                "us-east-1".into(),
            ];
            for c in 0..=127u8 {
                let mut s = baseline.to_owned();
                s.replace_range(..1, &(c as char).to_string());
                values.push(s);
            }
            for value in values {
                let wire = json!(value);
                let ok = match (schema, field) {
                    ("ProjectSecretsListRequest", _) => {
                        serde_json::from_value::<ProjectId>(wire.clone()).is_ok()
                    }
                    ("ProjectSecretSetRequest", "name") => {
                        serde_json::from_value::<SecretName>(wire.clone()).is_ok()
                    }
                    ("ProjectSecretSetRequest", _) => {
                        serde_json::from_value::<SecretRequestId>(wire.clone()).is_ok()
                    }
                    ("VaultLoginRequest", "requestId") => {
                        serde_json::from_value::<VaultRequestId>(wire.clone()).is_ok()
                    }
                    ("VaultLoginRequest", _) => {
                        serde_json::from_value::<AwsProfileName>(wire.clone()).is_ok()
                    }
                    ("VaultSetupRequest", "prefix") => serde_json::from_value::<
                        studio_runtime_schema::resource_ref::VaultPrefix,
                    >(wire.clone())
                    .is_ok(),
                    ("VaultSetupRequest", _) => {
                        serde_json::from_value::<Region>(wire.clone()).is_ok()
                    }
                    ("AccountGroupRequest", _) => serde_json::from_value::<
                        Option<studio_runtime_schema::project_ingress::AccountGroup>,
                    >(wire.clone())
                    .is_ok(),
                    ("RenameServer", _) => serde_json::from_value::<
                        BoundedPattern<
                            studio_runtime_schema::multi_server_ingress::ServerLabel,
                            1,
                            80,
                        >,
                    >(wire.clone())
                    .is_ok(),
                    ("ClaudeCancelRequest", _) => {
                        serde_json::from_value::<BoundedPattern<Uuid, 36, 36>>(wire.clone()).is_ok()
                    }
                    _ => serde_json::from_value::<Option<BoundedPattern<NonWhitespace, 1, 32>>>(
                        wire.clone(),
                    )
                    .is_ok(),
                };
                assert_eq!(ok, validator.is_valid(&wire), "{schema}.{field}: {wire:?}");
            }
        }
    }
}
