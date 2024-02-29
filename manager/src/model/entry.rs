use aib_cdx::entry::Entry as CdxEntry;
use aib_core::{
    digest::{Digest, Sha1Digest},
    timestamp::Timestamp,
};
use sqlx::{ColumnIndex, Decode, FromRow, Row, Type};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Entry {
    pub id: u64,
    pub surt_id: u64,
    pub entry: CdxEntry,
}

impl<'r, R: Row> FromRow<'r, R> for Entry
where
    for<'a> &'a str: ColumnIndex<R>,
    i64: Decode<'r, R::Database>,
    i64: Type<R::Database>,
    i32: Decode<'r, R::Database>,
    i32: Type<R::Database>,
    &'r str: Decode<'r, R::Database>,
    &'r str: Type<R::Database>,
{
    fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
        let entry_id = row.try_get::<i64, _>("entry_id")?;
        let surt_id = row.try_get::<i64, _>("surt_id")?;
        let surt_str = row.try_get::<&str, _>("surt")?;
        let timestamp = row.try_get::<i64, _>("timestamp")?;
        let url = row.try_get::<&str, _>("url")?;
        let mime_type = row.try_get::<&str, _>("mime_type")?;
        let status_code = row.try_get::<Option<i32>, _>("status_code")?;
        let digest = row.try_get::<&str, _>("digest")?;
        let length = row.try_get::<i64, _>("length")?;

        Ok(Self {
            id: super::try_cast(entry_id)?,
            surt_id: super::try_cast(surt_id)?,
            entry: CdxEntry {
                key: surt_str
                    .parse()
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
                timestamp: timestamp
                    .try_into()
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
                original: url.to_string(),
                mime_type: mime_type
                    .parse()
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
                status_code: status_code
                    .map(super::try_cast)
                    .map_or_else(|| Ok(None), |result| result.map(Some))?,
                digest: digest
                    .parse()
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
                length: super::try_cast(length)?,
                extra_info: None,
            },
        })
    }
}

/// Invalid digest for a CDX entry.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
pub struct InvalidDigest {
    pub url: String,
    pub timestamp: Timestamp,
    pub expected: Digest,
    pub actual: Sha1Digest,
}

impl<'r, R: Row> FromRow<'r, R> for InvalidDigest
where
    for<'a> &'a str: ColumnIndex<R>,
    i64: Decode<'r, R::Database>,
    i64: Type<R::Database>,
    &'r str: Decode<'r, R::Database>,
    &'r str: Type<R::Database>,
{
    fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
        let url = row.try_get::<&str, _>("url")?;
        let timestamp = row.try_get::<i64, _>("timestamp")?;
        let expected = row.try_get::<&str, _>("expected")?;
        let actual = row.try_get::<&str, _>("actual")?;

        Ok(Self {
            url: url.to_string(),
            timestamp: timestamp
                .try_into()
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
            expected: expected
                .parse()
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
            actual: actual
                .parse()
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
        })
    }
}
