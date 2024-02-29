use aib_core::{digest::Digest, entry::UrlParts, timestamp::Timestamp};
use aib_indexer::query::Range;
use chrono::{DateTime, Utc};
use sqlx::{query, query_as, query_scalar, Acquire, Executor, Row, Sqlite, SqliteConnection};
use std::collections::HashMap;

pub mod entry;
pub mod model;
pub mod pattern;
pub mod snapshot;
pub mod surt;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("SQL error")]
    Sqlx(#[from] sqlx::Error),
    #[error("Digest error")]
    Digest(#[from] aib_core::digest::Error),
    #[error("Invalid SURT")]
    Surt(#[from] aib_core::surt::Error),
    #[error("Invalid ID")]
    InvalidId(u64),
    #[error("Invalid digest")]
    InvalidDigest(Digest),
    #[error("Invalid timestamp")]
    InvalidTimestamp(i64),
}

pub struct Db<'a> {
    pub connection: &'a mut SqliteConnection,
}

impl<'a> Db<'a> {
    pub fn new(connection: &'a mut SqliteConnection) -> Self {
        Self { connection }
    }

    pub async fn get_search_result(
        &mut self,
        date_range: &Option<Range<DateTime<Utc>>>,
        snapshot_ids: &[i64],
    ) -> Result<
        (
            Vec<(i64, UrlParts, model::Surt)>,
            HashMap<i64, Vec<Timestamp>>,
        ),
        Error,
    > {
        let mut tx = self.connection.begin().await?;

        let snapshots = Self::get_snapshots(&mut *tx, snapshot_ids).await?;
        let surt_ids = snapshots
            .iter()
            .map(|(_, _, surt)| surt.id)
            .collect::<Vec<_>>();

        let surt_entries = Self::get_surt_entries(&mut *tx, &surt_ids, date_range).await?;

        tx.commit().await?;

        Ok((snapshots, surt_entries))
    }

    async fn get_surt_entries<'c, E: Executor<'c, Database = Sqlite>>(
        executor: E,
        surt_ids: &[i64],
        date_range: &Option<Range<DateTime<Utc>>>,
    ) -> Result<HashMap<i64, Vec<Timestamp>>, Error> {
        // TODO: Use macro if SQLx begins supporting sequence binding for SQLite.
        let query_string = format!(
            "SELECT
              surt_id,
              entry.ts
            FROM entry
            JOIN entry_success ON entry_success.entry_id == entry.id
            WHERE surt_id IN ({})
            {}
            ORDER BY surt_id, entry.ts",
            surt_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", "),
            date_range
                .as_ref()
                .map(|_| "AND entry.ts >= ? AND entry.ts < ?")
                .unwrap_or_default()
        );
        let mut query = sqlx::query(&query_string);

        for surt_id in surt_ids {
            query = query.bind(surt_id);
        }

        let timestamp_range = date_range.map(|range| range.map(|value| value.timestamp()));

        if let Some(range) = timestamp_range {
            let start_timestamp = range.start().unwrap_or(&i64::MIN);
            let end_timestamp = range.end().unwrap_or(&i64::MAX);
            query = query.bind(*start_timestamp).bind(*end_timestamp);
        }

        let rows = query.fetch_all(executor).await?;

        let mut results: HashMap<i64, Vec<Timestamp>> = HashMap::new();

        for row in rows {
            let surt_id = row.get::<i64, _>("surt_id");
            let timestamp = row.get::<i64, _>("ts");
            let timestamp = Timestamp(
                DateTime::from_timestamp(timestamp, 0)
                    .ok_or_else(|| Error::InvalidTimestamp(timestamp))?,
            );

            let entry = results.entry(surt_id).or_default();
            entry.push(timestamp);
        }

        Ok(results)
    }

    async fn get_snapshots<'c, E: Executor<'c, Database = Sqlite>>(
        executor: E,
        snapshot_ids: &[i64],
    ) -> Result<Vec<(i64, UrlParts, model::Surt)>, Error> {
        // TODO: Use macro if SQLx begins supporting sequence binding for SQLite.
        let query_string = format!(
            "SELECT
                entry_success.snapshot_id,
                entry.url,
                entry.ts,
                entry.surt_id,
                surt.value AS surt_value
            FROM entry_success
            JOIN entry ON entry.id = entry_success.entry_id
            JOIN surt ON surt.id = entry.surt_id
            WHERE entry_success.snapshot_id IN ({})
        ",
            snapshot_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut query = sqlx::query(&query_string);

        for snapshot_id in snapshot_ids {
            query = query.bind(snapshot_id);
        }

        let rows = query.fetch_all(executor).await?;

        let results = rows
            .into_iter()
            .map(|row| {
                let snapshot_id = row.get::<i64, _>("snapshot_id");
                let url = row.get::<String, _>("url");
                let timestamp = row.get::<i64, _>("ts");
                let surt_id = row.get::<i64, _>("surt_id");
                let surt_value = row.get::<String, _>("surt_value");
                Ok((
                    snapshot_id,
                    UrlParts::new(
                        url,
                        Timestamp(
                            DateTime::from_timestamp(timestamp, 0)
                                .ok_or_else(|| Error::InvalidTimestamp(timestamp))?,
                        ),
                    ),
                    model::Surt {
                        id: surt_id,
                        value: surt_value,
                    },
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(results)
    }

    pub async fn get_snapshot_info(
        &mut self,
        mime_type: &str,
    ) -> Result<Vec<(i64, i64, String, String, DateTime<Utc>)>, Error> {
        let rows = query!(
            "SELECT
                snapshot.id AS snapshot_id,
                entry.surt_id AS surt_id,
                snapshot.digest AS digest,
                pattern.slug AS pattern_slug,
                entry.ts AS timestamp
            FROM snapshot
            JOIN entry_success ON entry_success.snapshot_id = snapshot.id
            JOIN entry ON entry.id = entry_success.entry_id
            JOIN pattern_entry ON pattern_entry.entry_id = entry.id
            JOIN pattern on pattern.id = pattern_entry.pattern_id
            WHERE entry.mime_type = ?
            ORDER BY snapshot_id, timestamp
            ",
            mime_type
        )
        .fetch_all(&mut *self.connection)
        .await?;

        let results = rows
            .into_iter()
            .map(|record| {
                Ok((
                    record.snapshot_id,
                    record.surt_id,
                    record.pattern_slug,
                    record.digest,
                    DateTime::from_timestamp(record.timestamp, 0)
                        .ok_or_else(|| Error::InvalidTimestamp(record.timestamp))?,
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(results)
    }

    pub async fn get_entries_by_digest(
        &mut self,
        digest: &str,
    ) -> Result<Vec<model::Entry>, Error> {
        Ok(query_as!(
            model::Entry,
            "SELECT
                surt.id AS id,
                url,
                surt.id AS surt_id,
                surt.value AS surt,
                ts,
                digest,
                mime_type,
                status_code,
                length
            FROM entry
            JOIN surt ON surt.id = entry.surt_id
            WHERE digest = ?
            ",
            digest
        )
        .fetch_all(&mut *self.connection)
        .await?)
    }

    pub async fn missing_entries(&mut self, mime_type: &str) -> Result<Vec<model::Entry>, Error> {
        let entries = query_as!(
            model::Entry,
            "SELECT
                entry.id AS id,
                url,
                surt.id AS surt_id,
                surt.value AS surt,
                entry.ts AS ts,
                digest,
                mime_type,
                entry.status_code AS status_code,
                length
            FROM entry
            LEFT JOIN entry_success ON entry_success.entry_id = entry.id
            JOIN surt ON surt.id = entry.surt_id
            WHERE mime_type = ? AND entry_success.id IS NULL AND (entry.status_code IS NULL OR entry.status_code == 200) 
            ",
            mime_type,
        )
        .fetch_all(&mut *self.connection)
        .await?;

        Ok(entries)
    }

    async fn set_pattern_updated<'c, E: Executor<'c, Database = Sqlite>>(
        executor: E,
        id: i64,
        updated: DateTime<Utc>,
    ) -> Result<bool, Error> {
        let timestamp = updated.timestamp();
        Ok(query_scalar!(
            "UPDATE pattern SET updated = ? WHERE id = ? RETURNING id",
            timestamp,
            id
        )
        .persistent(true)
        .fetch_optional(executor)
        .await?
        .is_some())
    }

    async fn insert_link<'c, E: Executor<'c, Database = Sqlite>>(
        executor: E,
        url: &str,
        surt: &str,
    ) -> Result<i64, Error> {
        Ok(query_scalar!(
            "INSERT INTO link(url, surt)
                VALUES (?, ?) ON CONFLICT DO UPDATE SET id = id RETURNING id",
            url,
            surt
        )
        .persistent(true)
        .fetch_one(executor)
        .await?)
    }

    async fn insert_snapshot_link<'c, E: Executor<'c, Database = Sqlite>>(
        executor: E,
        snapshot_id: i64,
        link_id: i64,
    ) -> Result<(), Error> {
        query!(
            "INSERT OR IGNORE INTO snapshot_link(snapshot_id, link_id) VALUES (?, ?)",
            snapshot_id,
            link_id
        )
        .persistent(true)
        .execute(executor)
        .await?;

        Ok(())
    }
}
