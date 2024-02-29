use aib_core::{digest::Sha1Digest, entry::UrlParts, timestamp::Timestamp};
use chrono::DateTime;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub id: i64,
    pub url: String,
    pub surt_id: i64,
    pub surt: String,
    pub ts: i64,
    pub digest: String,
    pub mime_type: String,
    pub status_code: Option<i64>,
    pub length: i64,
}

impl Entry {
    pub fn url_parts(&self) -> Result<UrlParts, super::Error> {
        Ok(UrlParts::new(
            self.url.clone(),
            Timestamp(
                DateTime::from_timestamp(self.ts, 0)
                    .ok_or_else(|| super::Error::InvalidTimestamp(self.ts))?,
            ),
        ))
    }

    pub fn timestamp(&self) -> Result<Timestamp, super::Error> {
        Ok(Timestamp(
            DateTime::from_timestamp(self.ts, 0)
                .ok_or_else(|| super::Error::InvalidTimestamp(self.ts))?,
        ))
    }

    pub fn digest(&self) -> Result<Sha1Digest, super::Error> {
        Ok(self.digest.parse()?)
    }

    pub fn surt(&self) -> Surt {
        Surt {
            id: self.surt_id,
            value: self.surt.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct Surt {
    pub id: i64,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub id: i64,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryResult {
    pub id: i64,
    pub entry: Entry,
    pub snapshot: Snapshot,
    pub ts: i64,
    pub status_code: Option<i32>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Link {
    pub id: i64,
    pub url: String,
    pub surt: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotLink {
    pub snapshot: Snapshot,
    pub link: Link,
}
