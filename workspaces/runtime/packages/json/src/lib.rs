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

/// Serde's private wire marker for raw JSON values.
pub const RAW_VALUE_TOKEN: &str = "$serde_json::private::RawValue";
/// Serde JSON's `number.rs` marker, emitted when its `arbitrary_precision` feature is enabled.
pub const NUMBER_TOKEN: &str = "$serde_json::private::Number";

#[cfg(not(feature = "sonic-backend"))]
mod raw;
#[cfg(not(feature = "sonic-backend"))]
pub use raw::{OwnedRawJson, RawJson};
#[cfg(feature = "sonic-backend")]
mod raw_sonic;
#[cfg(feature = "sonic-backend")]
pub use raw_sonic::{OwnedRawJson, RawJson};

/// Process-local facade call counts, grouped by operation and Rust type.
#[cfg(feature = "perf-reporting")]
pub mod perf_reporting {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};
    /// Maximum retained operation/type labels, excluding the overflow bucket.
    const MAX_KEYS: usize = 4096;
    /// Maximum operation/type label length in bytes.
    const MAX_KEY_BYTES: usize = 2048;
    #[derive(Clone, Default, serde::Serialize)]
    pub struct Counts {
        pub calls: u64,
        pub errors: u64,
    }
    static COUNTS: OnceLock<Mutex<BTreeMap<String, Counts>>> = OnceLock::new();
    thread_local! { static ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    struct ActiveGuard;
    impl Drop for ActiveGuard {
        fn drop(&mut self) {
            ACTIVE.with(|active| active.set(false));
        }
    }
    pub(super) fn measure<T: ?Sized, R>(
        operation: &str,
        f: impl FnOnce() -> super::Result<R>,
    ) -> super::Result<R> {
        if ACTIVE.with(|active| active.replace(true)) {
            return f();
        }
        let _guard = ActiveGuard;
        let result = f();
        let label = format!("{} {}", operation, std::any::type_name::<T>());
        let mut counts = COUNTS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key = if label.len() <= MAX_KEY_BYTES
            && (counts.contains_key(&label) || counts.len() < MAX_KEYS)
        {
            label.as_str()
        } else {
            "<other>"
        };
        let entry = counts.entry(key.to_owned()).or_default();
        entry.calls = entry.calls.saturating_add(1);
        if result.is_err() {
            entry.errors = entry.errors.saturating_add(1);
        }
        result
    }
    pub fn snapshot() -> BTreeMap<String, Counts> {
        COUNTS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[cfg(feature = "perf-reporting")]
macro_rules! measure_json {
    ($ty:ty, $operation:literal, $body:block) => {
        perf_reporting::measure::<$ty, _>($operation, || $body)
    };
}
#[cfg(not(feature = "perf-reporting"))]
macro_rules! measure_json {
    ($ty:ty, $operation:literal, $body:block) => {
        $body
    };
}

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
    measure_json!(T, "from_str", {
        #[cfg(feature = "sonic-backend")]
        {
            sonic_rs::from_str(input).map_err(error)
        }
        #[cfg(not(feature = "sonic-backend"))]
        {
            serde_json::from_str(input).map_err(error)
        }
    })
}
#[inline]
pub fn from_slice<'a, T: Deserialize<'a>>(input: &'a [u8]) -> Result<T> {
    measure_json!(T, "from_slice", {
        #[cfg(feature = "sonic-backend")]
        {
            sonic_rs::from_slice(input).map_err(error)
        }
        #[cfg(not(feature = "sonic-backend"))]
        {
            serde_json::from_slice(input).map_err(error)
        }
    })
}
#[inline]
pub fn from_str_owned<T: DeserializeOwned>(input: &str) -> Result<T> {
    measure_json!(T, "from_str_owned", { from_slice_owned(input.as_bytes()) })
}
#[inline]
pub fn from_slice_owned<T: DeserializeOwned>(input: &[u8]) -> Result<T> {
    measure_json!(T, "from_slice_owned", {
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
    })
}
#[inline]
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    measure_json!(T, "to_string", {
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
    })
}
#[inline]
pub fn to_vec<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    measure_json!(T, "to_vec", {
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
    })
}
#[inline]
pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T> {
    measure_json!(T, "from_value", {
        serde_json::from_value(value).map_err(error)
    })
}
#[inline]
pub fn to_value<T: Serialize>(value: T) -> Result<Value> {
    measure_json!(T, "to_value", {
        serde_json::to_value(value).map_err(error)
    })
}
#[inline]
pub fn from_reader<R: std::io::Read, T: DeserializeOwned>(reader: R) -> Result<T> {
    measure_json!(T, "from_reader", {
        #[cfg(feature = "sonic-backend")]
        {
            sonic_rs::from_reader(reader).map_err(error)
        }
        #[cfg(not(feature = "sonic-backend"))]
        {
            serde_json::from_reader(reader).map_err(error)
        }
    })
}
#[inline]
pub fn to_writer<W: std::io::Write, T: Serialize + ?Sized>(writer: W, value: &T) -> Result<()> {
    measure_json!(T, "to_writer", {
        #[cfg(feature = "sonic-backend")]
        {
            sonic_rs::to_writer(sonic_rs::writer::BufferedWriter::new(writer), value).map_err(error)
        }
        #[cfg(not(feature = "sonic-backend"))]
        {
            serde_json::to_writer(writer, value).map_err(error)
        }
    })
}
#[inline]
pub fn to_string_pretty<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    measure_json!(T, "to_string_pretty", {
        #[cfg(feature = "sonic-backend")]
        {
            sonic_rs::to_string_pretty(value).map_err(error)
        }
        #[cfg(not(feature = "sonic-backend"))]
        {
            serde_json::to_string_pretty(value).map_err(error)
        }
    })
}
#[inline]
pub fn to_vec_pretty<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    measure_json!(T, "to_vec_pretty", {
        #[cfg(feature = "sonic-backend")]
        {
            sonic_rs::to_vec_pretty(value).map_err(error)
        }
        #[cfg(not(feature = "sonic-backend"))]
        {
            serde_json::to_vec_pretty(value).map_err(error)
        }
    })
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
    measure_json!(T, "to_raw_value", {
        #[cfg(feature = "serde-backend")]
        {
            serde_json::value::to_raw_value(value)
                .map(OwnedRawJson::from_raw)
                .map_err(Error::from)
        }
        #[cfg(not(feature = "serde-backend"))]
        {
            OwnedRawJson::from_string(to_string(value)?)
        }
    })
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

#[cfg(all(test, feature = "perf-reporting"))]
mod perf_tests {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct UniqueType {
        value: u32,
    }
    #[test]
    fn facade_counts_outer_calls_and_errors_by_type() {
        let _ = super::from_str_owned::<UniqueType>("{\"value\":1}").unwrap();
        assert!(super::from_str_owned::<UniqueType>("invalid").is_err());
        let _ = super::to_string(&UniqueType { value: 1 }).unwrap();
        let counts = super::perf_reporting::snapshot();
        let key = format!("from_str_owned {}", std::any::type_name::<UniqueType>());
        assert_eq!(counts[&key].calls, 2);
        assert_eq!(counts[&key].errors, 1);
        let nested = format!("from_slice {}", std::any::type_name::<UniqueType>());
        assert!(!counts.contains_key(&nested));
        let encoded = format!("to_string {}", std::any::type_name::<UniqueType>());
        assert_eq!(counts[&encoded].calls, 1);
    }
}
