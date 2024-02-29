use crate::model::entry::InvalidDigest;
use aib_cdx::entry::Entry as CdxEntry;
use chrono::{DateTime, Utc};
use sqlx::{query_as, query_scalar, Connection, Executor, Sqlite, SqliteConnection};

pub async fn insert<'c>(
    connection: &mut SqliteConnection,
    entry: &CdxEntry,
) -> Result<u64, sqlx::Error> {
    let surt_id = crate::db::surt::insert(&mut *connection, &entry.key.to_string()).await?;
    let entry_id = insert_entry(&mut *connection, entry, surt_id).await?;

    Ok(entry_id)
}

pub async fn insert_entry<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
    entry: &CdxEntry,
    surt_id: u64,
) -> Result<u64, sqlx::Error> {
    let surt_id = surt_id as i64;
    let timestamp = entry.timestamp.0.timestamp();
    let digest = entry.digest.to_string();
    let mime_type = entry.mime_type.to_string();
    let status_code = entry.status_code.map(|value| value as i32);
    let length = entry.length as i64;

    let id = query_scalar!(
        "INSERT INTO entry(url, surt_id, ts, digest, mime_type, status_code, length)
            VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT DO UPDATE SET id = id RETURNING id",
        entry.original,
        surt_id,
        timestamp,
        digest,
        mime_type,
        status_code,
        length
    )
    .persistent(true)
    .fetch_one(executor)
    .await?;

    Ok(id as u64)
}

pub async fn insert_entry_success(
    connection: &mut SqliteConnection,
    entry_id: u64,
    digest: &str,
    correct_digest: bool,
    timestamp: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let mut tx = connection.begin().await?;

    let entry_id = entry_id as i64;
    let snapshot_id = crate::db::snapshot::insert(&mut *tx, digest).await?;
    let timestamp = timestamp.timestamp();

    let id = query_scalar!(
        "INSERT INTO entry_success(entry_id, snapshot_id, correct_digest, ts) VALUES (?, ?, ?, ?)
            ON CONFLICT DO UPDATE SET id = id RETURNING id",
        entry_id,
        snapshot_id,
        correct_digest,
        timestamp
    )
    .persistent(true)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(id as u64)
}

pub async fn insert_entry_error(
    connection: &mut SqliteConnection,
    entry_id: i64,
    timestamp: DateTime<Utc>,
    status_code: u16,
    error_message: &str,
) -> Result<i64, sqlx::Error> {
    let timestamp = timestamp.timestamp();
    let status_code = status_code as i32;

    let id = query_scalar!(
        "INSERT INTO entry_failure(entry_id, ts, status_code, error_message) VALUES (?, ?, ?, ?) RETURNING id",
        entry_id,
        timestamp,
        status_code,
        error_message
    )
    .persistent(true)
    .fetch_one(&mut *connection)
    .await?;

    Ok(id)
}

pub async fn missing_entries<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
    mime_type: &str,
    count: Option<usize>,
) -> Result<Vec<crate::model::Entry>, sqlx::Error> {
    let count = count.map(|value| value as i32).unwrap_or(i32::MAX);

    query_as(
        "SELECT
            entry.id AS entry_id,
            surt.id AS surt_id,
            surt.value AS surt,
            entry.ts AS timestamp,
            url,
            mime_type,
            entry.status_code AS status_code,
            digest,
            length
        FROM entry
        LEFT JOIN entry_success ON entry_success.entry_id = entry.id
        JOIN surt ON surt.id = entry.surt_id
        WHERE mime_type = ? AND entry_success.id IS NULL AND (entry.status_code IS NULL OR entry.status_code == 200) 
        LIMIT ?
        ",
    )
    .bind(mime_type)
    .bind(count)
    .persistent(true)
    .fetch_all(executor)
    .await
}

pub async fn invalid_digests<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
) -> Result<Vec<InvalidDigest>, sqlx::Error> {
    query_as(
        "SELECT entry.url AS url, entry.ts AS timestamp, entry.digest AS expected, snapshot.digest AS actual
        FROM entry_success
        JOIN entry on entry.id = entry_success.entry_id
        JOIN snapshot on snapshot.id = entry_success.snapshot_id
        WHERE NOT correct_digest
        ORDER BY url, timestamp"
    )
    .persistent(true)
    .fetch_all(executor)
    .await
}

pub async fn find_entries_by_digest<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
    digest: &str,
) -> Result<Vec<u64>, sqlx::Error> {
    let ids: Vec<i64> = query_scalar!("SELECT entry.id FROM entry WHERE digest = ?", digest)
        .persistent(true)
        .fetch_all(executor)
        .await?;

    ids.into_iter()
        .map(|value| {
            value
                .try_into()
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aib_cdx::entry::Entry as CdxEntry;
    use aib_core::surt::Surt;
    use chrono::Duration;
    use sqlx::SqlitePool;

    #[sqlx::test]
    async fn test_insert(pool: SqlitePool) -> Result<(), sqlx::Error> {
        let mut connection = pool.acquire().await?;

        let (id_0, id_1, id_2) = insert_entries(&mut *connection).await?;

        assert_eq!(id_0, 1);
        assert_eq!(id_1, 2);
        assert_eq!(id_2, 1);

        Ok(())
    }

    async fn insert_entries<'a>(
        connection: &mut SqliteConnection,
    ) -> Result<(u64, u64, u64), sqlx::Error> {
        let url = "https://test.com/";
        let surt = Surt::from_url(url).unwrap();
        let now_0 = Utc::now();
        let now_1 = now_0 + Duration::seconds(10);

        let id_0 = insert(
            &mut *connection,
            &CdxEntry {
                key: surt.clone(),
                timestamp: aib_core::timestamp::Timestamp(now_0),
                original: url.to_string(),
                mime_type: "text/html".parse().unwrap(),
                status_code: Some(200),
                digest: "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".parse().unwrap(),
                length: 987654,
                extra_info: None,
            },
        )
        .await?;
        let id_1 = insert(
            &mut *connection,
            &CdxEntry {
                key: surt.clone(),
                timestamp: aib_core::timestamp::Timestamp(now_1),
                original: url.to_string(),
                mime_type: "text/html".parse().unwrap(),
                status_code: Some(200),
                digest: "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".parse().unwrap(),
                length: 10000,
                extra_info: None,
            },
        )
        .await?;
        let id_2 = insert(
            &mut *connection,
            &CdxEntry {
                key: surt.clone(),
                timestamp: aib_core::timestamp::Timestamp(now_0),
                original: url.to_string(),
                mime_type: "text/html".parse().unwrap(),
                status_code: Some(404),
                digest: "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".parse().unwrap(),
                length: 123,
                extra_info: None,
            },
        )
        .await?;

        Ok((id_0, id_1, id_2))
    }
}
