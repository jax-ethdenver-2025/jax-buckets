use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

use common::prelude::{multibase::Base, Cid, Link};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
#[serde(transparent)]
pub struct DCid(Cid);

impl Default for DCid {
    fn default() -> Self {
        Self(Link::default().into())
    }
}

impl From<DCid> for Link {
    fn from(val: DCid) -> Self {
        Link::from(val.0)
    }
}

impl From<Link> for DCid {
    fn from(link: Link) -> Self {
        Self(Cid::from(link))
    }
}

impl From<DCid> for Cid {
    fn from(val: DCid) -> Self {
        val.0
    }
}

impl From<Cid> for DCid {
    fn from(cid: Cid) -> Self {
        Self(cid)
    }
}

impl Decode<'_, Sqlite> for DCid {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let db_val = <String as Decode<Sqlite>>::decode(value)?;
        let cid = Cid::try_from(db_val).map_err(DCidError::InvalidCid)?;

        Ok(Self(cid))
    }
}

impl Encode<'_, Sqlite> for DCid {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'_>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(
            self.0.to_string_of_base(Base::Base32Lower).unwrap().into(),
        ));
        Ok(IsNull::No)
    }
}

impl Type<Sqlite> for DCid {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DCidError {
    #[error("invalid cid: {0}")]
    InvalidCid(#[from] common::prelude::CidError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_cid() -> Result<(), BoxDynError> {
        // Create a test CID
        let test_str = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        let cid = Cid::try_from(test_str).unwrap();
        let dcid = DCid(cid);

        // Test encoding
        let mut args = Vec::new();
        let _ = dcid.encode_by_ref(&mut args)?;

        // Verify encoded value
        if let SqliteArgumentValue::Text(encoded) = &args[0] {
            assert_eq!(encoded.as_ref(), test_str);
        } else {
            panic!("Expected Text variant");
        }

        Ok(())
    }
}
