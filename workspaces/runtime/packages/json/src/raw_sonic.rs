use crate::{Error, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[repr(transparent)]
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RawJson<'a>(#[serde(borrow)] sonic_rs::LazyValue<'a>);

#[repr(transparent)]
#[derive(Clone)]
pub struct OwnedRawJson(Box<str>);

impl RawJson<'_> {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_raw_str()
    }
    #[inline]
    pub fn to_owned(&self) -> OwnedRawJson {
        OwnedRawJson(self.as_str().into())
    }
}
impl OwnedRawJson {
    pub fn from_string(input: String) -> Result<Self> {
        let raw = sonic_rs::from_str::<sonic_rs::LazyValue<'_>>(&input).map_err(Error::from)?;
        Ok(Self(raw.as_raw_str().into()))
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for OwnedRawJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let raw = sonic_rs::LazyValue::deserialize(deserializer)?;
        Ok(Self(raw.as_raw_str().into()))
    }
}
impl Serialize for OwnedRawJson {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let raw = sonic_rs::from_str::<sonic_rs::LazyValue<'_>>(self.as_str())
            .map_err(serde::ser::Error::custom)?;
        raw.serialize(serializer)
    }
}
impl fmt::Debug for RawJson<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}
impl fmt::Debug for OwnedRawJson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}
impl PartialEq for RawJson<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
impl PartialEq for OwnedRawJson {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
