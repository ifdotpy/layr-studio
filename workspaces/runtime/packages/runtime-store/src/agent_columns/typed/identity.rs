use studio_db_api::migration::{decode_key, encode_key};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AgentIdentityError {
    #[error("invalid agent migration key frame")]
    InvalidFrame,
    #[error("invalid agent identity component shape")]
    InvalidShape,
    #[error("agent identity frame is not canonical")]
    Noncanonical,
}

/// Source discriminator used only to distinguish NULL legacy keys.
/// Nonnull agent/chat/live keys share their original logical identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NullableAgentSource {
    Agent,
    Volatile,
}

impl NullableAgentSource {
    const fn tie(self) -> i64 {
        match self {
            Self::Agent => 0,
            Self::Volatile => 1,
        }
    }
}

type FramedAgentKey = (Option<String>, Option<i64>, Option<i64>);

/// Canonical physical identity for merged agents and effective live rows.
/// The frame is the existing MigrationKey format, never an ordering key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AgentIdentity(Vec<u8>);

impl rusqlite::types::ToSql for AgentIdentity {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(
            rusqlite::types::ValueRef::Blob(self.as_bytes()),
        ))
    }
}

impl rusqlite::types::FromSql for AgentIdentity {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Self::from_bytes(value.as_blob()?)
            .map_err(|error| rusqlite::types::FromSqlError::Other(Box::new(error)))
    }
}

impl studio_db_api::DbValue for AgentIdentity {
    const SQL_TYPE: &'static str = "BLOB";
}

impl AgentIdentity {
    pub fn for_agent(id: &crate::AgentId) -> Self {
        Self::from_source(NullableAgentSource::Agent, Some(id.as_ref()), 0)
    }

    /// Preserve NULL-key multiplicity without equating NULL base and live rows.
    pub fn from_source(source: NullableAgentSource, id: Option<&str>, rowid: i64) -> Self {
        let key = match id {
            Some(id) => (Some(id.to_owned()), None, None),
            None => (None, Some(source.tie()), Some(rowid)),
        };
        Self(encode_key(&key))
    }

    /// Dirty triggers encode the original nullable id and physical rowid.
    /// Normalize only after decoding that source cursor, using its own stream.
    pub fn from_dirty_key(
        source: NullableAgentSource,
        cursor: &[u8],
    ) -> Result<Self, AgentIdentityError> {
        let (id, rowid) = decode_key::<(Option<String>, i64)>(cursor)
            .map_err(|_| AgentIdentityError::InvalidFrame)?;
        Ok(Self::from_source(source, id.as_deref(), rowid))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AgentIdentityError> {
        let key =
            decode_key::<FramedAgentKey>(bytes).map_err(|_| AgentIdentityError::InvalidFrame)?;
        match &key {
            (Some(_), None, None) | (None, Some(0 | 1), Some(_)) => {}
            _ => return Err(AgentIdentityError::InvalidShape),
        }
        // Reject alternative integer spellings and any noncanonical encoding.
        if encode_key(&key) != bytes {
            return Err(AgentIdentityError::Noncanonical);
        }
        Ok(Self(bytes.to_vec()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
