use kaspa_hashes::Hash;
use sqlx;
use sqlx::encode::IsNull;
use sqlx::postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueRef};
use sqlx::{Decode, Encode, Postgres, Type};

/// Wrapper type for kaspa_hashes::Hash implementing the SQLX Encode & Decode traits
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct SqlHash(Hash);

impl SqlHash {
    pub const fn as_bytes(&self) -> [u8; 32] {
        self.0.as_bytes()
    }
}

impl From<Hash> for SqlHash {
    fn from(hash: Hash) -> Self {
        SqlHash(hash)
    }
}

impl From<SqlHash> for Hash {
    fn from(sql_hash: SqlHash) -> Self {
        sql_hash.0
    }
}

impl Type<Postgres> for SqlHash {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("BYTEA")
    }
}

impl PgHasArrayType for SqlHash {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("_BYTEA")
    }
}

impl Encode<'_, Postgres> for SqlHash {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        buf.extend_from_slice(&self.0.as_bytes());
        IsNull::No
    }
}

impl<'r> Decode<'r, Postgres> for SqlHash {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bytes = value.as_bytes()?;
        let hash = Hash::from_slice(bytes);
        Ok(SqlHash(hash))
    }
}
