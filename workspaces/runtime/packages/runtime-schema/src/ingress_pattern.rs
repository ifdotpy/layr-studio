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
pub struct BoundedSafeInteger<const MIN: u64, const MAX: u64>(crate::recovery_record::SafeInteger);
impl<const MIN: u64, const MAX: u64> BoundedSafeInteger<MIN, MAX> {
    pub fn get(&self) -> i64 {
        self.0.get() as i64
    }
}
impl<'de, const MIN: u64, const MAX: u64> Deserialize<'de> for BoundedSafeInteger<MIN, MAX> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = crate::recovery_record::SafeInteger::deserialize(d)?;
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
