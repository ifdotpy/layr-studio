use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

#[repr(transparent)]
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RawJson<'a>(#[serde(borrow)] &'a serde_json::value::RawValue);

#[repr(transparent)]
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OwnedRawJson(Box<serde_json::value::RawValue>);

impl RawJson<'_> {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.get()
    }

    #[inline]
    pub fn to_owned(&self) -> OwnedRawJson {
        OwnedRawJson(self.0.to_owned())
    }
}
impl OwnedRawJson {
    #[inline]
    pub fn from_string(input: String) -> Result<Self> {
        serde_json::value::RawValue::from_string(input)
            .map(Self)
            .map_err(Error::from)
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.get()
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
