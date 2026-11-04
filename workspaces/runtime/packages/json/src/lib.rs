//! Studio's serde-compatible JSON entry points.

#[cfg(any(
    all(feature = "serde-backend", feature = "sonic-backend"),
    all(feature = "serde-backend", feature = "simd-backend"),
    all(feature = "sonic-backend", feature = "simd-backend"),
    not(any(
        feature = "serde-backend",
        feature = "sonic-backend",
        feature = "simd-backend"
    ))
))]
compile_error!("select exactly one JSON backend");

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt;

pub use serde_json::{Deserializer, Map, Number, Serializer, Value, error, json, value};

#[cfg(not(feature = "sonic-backend"))]
mod raw;
#[cfg(not(feature = "sonic-backend"))]
pub use raw::{OwnedRawJson, RawJson};
#[cfg(feature = "sonic-backend")]
mod raw_sonic;
#[cfg(feature = "sonic-backend")]
pub use raw_sonic::{OwnedRawJson, RawJson};

/// An error returned by a facade entry point.
#[derive(Debug)]
pub enum Error {
    Serde(serde_json::Error),
    #[cfg(feature = "sonic-backend")]
    Sonic(sonic_rs::Error),
    #[cfg(feature = "simd-backend")]
    Simd(simd_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serde(error) => error.fmt(f),
            #[cfg(feature = "sonic-backend")]
            Self::Sonic(error) => error.fmt(f),
            #[cfg(feature = "simd-backend")]
            Self::Simd(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serde(error) => Some(error),
            #[cfg(feature = "sonic-backend")]
            Self::Sonic(error) => Some(error),
            #[cfg(feature = "simd-backend")]
            Self::Simd(error) => Some(error),
        }
    }
}
impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self::Serde(<serde_json::Error as serde::de::Error>::custom(message))
    }
}
impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self::Serde(<serde_json::Error as serde::ser::Error>::custom(message))
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}
#[cfg(feature = "sonic-backend")]
impl From<sonic_rs::Error> for Error {
    fn from(value: sonic_rs::Error) -> Self {
        Self::Sonic(value)
    }
}
#[cfg(feature = "simd-backend")]
impl From<simd_json::Error> for Error {
    fn from(value: simd_json::Error) -> Self {
        Self::Simd(value)
    }
}
fn error<E: Into<Error>>(error: E) -> Error {
    error.into()
}

pub type Result<T> = std::result::Result<T, Error>;

#[inline]
pub fn from_str<'a, T: Deserialize<'a>>(input: &'a str) -> Result<T> {
    #[cfg(feature = "sonic-backend")]
    {
        sonic_rs::from_str(input).map_err(error)
    }
    #[cfg(not(feature = "sonic-backend"))]
    {
        serde_json::from_str(input).map_err(error)
    }
}
#[inline]
pub fn from_slice<'a, T: Deserialize<'a>>(input: &'a [u8]) -> Result<T> {
    #[cfg(feature = "sonic-backend")]
    {
        sonic_rs::from_slice(input).map_err(error)
    }
    #[cfg(not(feature = "sonic-backend"))]
    {
        serde_json::from_slice(input).map_err(error)
    }
}
#[inline]
pub fn from_str_owned<T: DeserializeOwned>(input: &str) -> Result<T> {
    from_slice_owned(input.as_bytes())
}
#[inline]
pub fn from_slice_owned<T: DeserializeOwned>(input: &[u8]) -> Result<T> {
    #[cfg(feature = "simd-backend")]
    {
        thread_local! { static SCRATCH: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) }; }
        SCRATCH.with(|scratch| {
            let mut scratch = scratch.borrow_mut();
            scratch.clear();
            scratch.extend_from_slice(input);
            simd_json::serde::from_slice(&mut scratch).map_err(error)
        })
    }
    #[cfg(not(feature = "simd-backend"))]
    {
        from_slice(input)
    }
}
#[inline]
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    #[cfg(feature = "sonic-backend")]
    {
        sonic_rs::to_string(value).map_err(error)
    }
    #[cfg(feature = "simd-backend")]
    {
        simd_json::serde::to_string(value).map_err(error)
    }
    #[cfg(feature = "serde-backend")]
    {
        serde_json::to_string(value).map_err(error)
    }
}
#[inline]
pub fn to_vec<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    #[cfg(feature = "sonic-backend")]
    {
        sonic_rs::to_vec(value).map_err(error)
    }
    #[cfg(feature = "simd-backend")]
    {
        simd_json::serde::to_vec(value).map_err(error)
    }
    #[cfg(feature = "serde-backend")]
    {
        serde_json::to_vec(value).map_err(error)
    }
}
#[inline]
pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(error)
}
#[inline]
pub fn to_value<T: Serialize>(value: T) -> Result<Value> {
    serde_json::to_value(value).map_err(error)
}
#[inline]
pub fn from_reader<R: std::io::Read, T: DeserializeOwned>(reader: R) -> Result<T> {
    #[cfg(feature = "sonic-backend")]
    {
        sonic_rs::from_reader(reader).map_err(error)
    }
    #[cfg(not(feature = "sonic-backend"))]
    {
        serde_json::from_reader(reader).map_err(error)
    }
}
#[inline]
pub fn to_writer<W: std::io::Write, T: Serialize + ?Sized>(writer: W, value: &T) -> Result<()> {
    #[cfg(feature = "sonic-backend")]
    {
        sonic_rs::to_writer(sonic_rs::writer::BufferedWriter::new(writer), value).map_err(error)
    }
    #[cfg(not(feature = "sonic-backend"))]
    {
        serde_json::to_writer(writer, value).map_err(error)
    }
}
#[inline]
pub fn to_string_pretty<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    #[cfg(feature = "sonic-backend")]
    {
        sonic_rs::to_string_pretty(value).map_err(error)
    }
    #[cfg(not(feature = "sonic-backend"))]
    {
        serde_json::to_string_pretty(value).map_err(error)
    }
}
#[inline]
pub fn to_vec_pretty<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    #[cfg(feature = "sonic-backend")]
    {
        sonic_rs::to_vec_pretty(value).map_err(error)
    }
    #[cfg(not(feature = "sonic-backend"))]
    {
        serde_json::to_vec_pretty(value).map_err(error)
    }
}

impl From<Error> for std::io::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::Serde(error) => error.into(),
            #[cfg(feature = "sonic-backend")]
            Error::Sonic(error) => std::io::Error::other(error),
            #[cfg(feature = "simd-backend")]
            Error::Simd(error) => std::io::Error::other(error),
        }
    }
}

#[inline]
pub fn to_raw_value<T: Serialize + ?Sized>(value: &T) -> Result<OwnedRawJson> {
    OwnedRawJson::from_string(to_string(value)?)
}

impl Error {
    #[inline]
    pub fn classify(&self) -> error::Category {
        match self {
            Self::Serde(error) => error.classify(),
            #[cfg(feature = "sonic-backend")]
            Self::Sonic(_) => error::Category::Data,
            #[cfg(feature = "simd-backend")]
            Self::Simd(_) => error::Category::Data,
        }
    }
}

impl Error {
    #[inline]
    pub fn is_io(&self) -> bool {
        self.classify() == error::Category::Io
    }

    #[inline]
    pub fn is_data(&self) -> bool {
        self.classify() == error::Category::Data
    }
}

impl Error {
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.classify() == error::Category::Eof
    }
    #[inline]
    pub fn is_syntax(&self) -> bool {
        self.classify() == error::Category::Syntax
    }
    #[inline]
    pub fn line(&self) -> usize {
        match self {
            Self::Serde(e) => e.line(),
            #[cfg(feature = "sonic-backend")]
            Self::Sonic(_) => 0,
            #[cfg(feature = "simd-backend")]
            Self::Simd(_) => 0,
        }
    }
    #[inline]
    pub fn column(&self) -> usize {
        match self {
            Self::Serde(e) => e.column(),
            #[cfg(feature = "sonic-backend")]
            Self::Sonic(_) => 0,
            #[cfg(feature = "simd-backend")]
            Self::Simd(_) => 0,
        }
    }
    #[inline]
    pub fn io(error: std::io::Error) -> Self {
        Self::Serde(serde_json::Error::io(error))
    }
}
