use crate::model::{entry::InvalidDigest, Entry, Pattern};
use aib_core::entry::{EntryInfo, UrlParts};
use chrono::Utc;
use itertools::Itertools;
use sqlx::{Connection, SqliteConnection};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("SQL error")]
    Sqlx(#[from] sqlx::Error),
    #[error("CDX store error")]
    CdxStore(#[from] aib_cdx_store::Error),
    #[error("JSON error")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PatternConfig {
    #[serde(flatten)]
    pub pattern: Pattern,
    pub path: PathBuf,
    #[serde(rename = "compression-level")]
    pub compression_level: Option<i32>,
}

pub async fn run_import<P: AsRef<Path>>(
    config_path: P,
    connection: &mut SqliteConnection,
) -> Result<usize, Error> {
    let config_file = File::open(config_path)?;
    let configs = serde_json::from_reader::<_, Vec<PatternConfig>>(BufReader::new(config_file))?;
    let mut count = 0;

    for config in configs {
        count += import_cdx_store(connection, &config).await?;
    }

    Ok(count)
}

pub async fn import_cdx_store(
    connection: &mut SqliteConnection,
    config: &PatternConfig,
) -> Result<usize, Error> {
    let count = 0;
    let store = aib_cdx_store::Store::new(&config.path, config.compression_level);

    let entries = store
        .entries()?
        .into_iter()
        .map(|(_timestamp, entry)| entry)
        .collect::<Vec<_>>();

    let mut tx = connection.begin().await?;
    let pattern_id = crate::db::pattern::insert(&mut *tx, &config.pattern).await?;

    for entry in entries {
        let entry_id = crate::db::entry::insert(&mut tx, &entry).await?;
        crate::db::pattern::insert_pattern_entry(&mut *tx, pattern_id, entry_id).await?;
    }

    tx.commit().await?;

    Ok(count)
}

pub async fn find_local_snapshots(
    connection: &mut SqliteConnection,
    store: &aib_store::items::ItemStore,
    mime_type: &str,
) -> Result<usize, Error> {
    let mut count = 0;

    let entries = crate::db::entry::missing_entries(&mut *connection, mime_type, None).await?;

    for Entry { id, entry, .. } in entries {
        let digest = entry.digest.to_string();
        if store.contains(&digest) {
            crate::db::entry::insert_entry_success(&mut *connection, id, &digest, true, Utc::now())
                .await?;

            count += 1;
        }
    }

    Ok(count)
}

pub async fn list_missing_snapshots(
    connection: &mut SqliteConnection,
    mime_type: &str,
) -> Result<Vec<EntryInfo>, Error> {
    let mut values = crate::db::entry::missing_entries(&mut *connection, mime_type, None)
        .await?
        .into_iter()
        .map(|Entry { entry, .. }| {
            Ok(EntryInfo {
                url_parts: UrlParts {
                    url: entry.original.to_string(),
                    timestamp: entry.timestamp,
                },
                expected_digest: entry.digest,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;

    // We want the shortest URL and then the earliest timestamp for each digest.
    values.sort_by_key(|entry| {
        (
            entry.expected_digest.clone(),
            entry.url_parts.url.len(),
            entry.url_parts.timestamp,
        )
    });

    let mut results = Vec::with_capacity(values.len());

    for (_, mut group) in &values
        .into_iter()
        .group_by(|entry| entry.expected_digest.clone())
    {
        // Safe because of guarantees provided by Itertools.
        results.push(group.next().unwrap());
    }

    results.sort();

    Ok(results)
}

/// Export a list of entries that were found to have invalid CDX digests.
pub async fn list_invalid_digests(
    connection: &mut SqliteConnection,
) -> Result<Vec<InvalidDigest>, sqlx::Error> {
    crate::db::entry::invalid_digests(&mut *connection).await
}

pub async fn import_invalid_digests(
    connection: &mut SqliteConnection,
    store: &aib_store::items::ItemStore,
    invalid_digests: &[InvalidDigest],
) -> Result<usize, sqlx::Error> {
    let mut count = 0;

    for InvalidDigest {
        expected, actual, ..
    } in invalid_digests
    {
        let expected_digest = expected.to_string();
        let actual_digest = actual.to_string();

        if !store.contains(&expected_digest) && store.contains(&actual_digest) {
            let entries =
                crate::db::entry::find_entries_by_digest(&mut *connection, &expected_digest)
                    .await?;

            for entry_id in entries {
                crate::db::entry::insert_entry_success(
                    &mut *connection,
                    entry_id,
                    &actual_digest,
                    false,
                    Utc::now(),
                )
                .await?;

                count += 1;
            }
        }
    }

    Ok(count)
}
