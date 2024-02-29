use crate::model::Pattern;
use sqlx::{query, query_as, query_scalar, Executor, Sqlite};

pub async fn get_all<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
) -> Result<Vec<Pattern>, sqlx::Error> {
    query_as(
        "
        SELECT
            pattern.id AS pattern_id,
            pattern.surt,
            prefix,
            name,
            slug,
            sort_id
        FROM pattern
        ORDER BY pattern.sort_id
    ",
    )
    .persistent(true)
    .fetch_all(executor)
    .await
}

pub async fn get_all_with_stats<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
) -> Result<Vec<Pattern>, sqlx::Error> {
    query_as(
        "
        SELECT
            pattern.id AS pattern_id,
            pattern.surt,
            prefix,
            name,
            slug,
            sort_id,
            COUNT(DISTINCT entry_success.entry_id) AS indexed_count
        FROM pattern
        LEFT JOIN pattern_entry ON pattern_entry.pattern_id = pattern.id
        LEFT JOIN entry_success ON entry_success.entry_id = pattern_entry.entry_id
        GROUP BY pattern.id
        ORDER BY pattern.sort_id
    ",
    )
    .persistent(true)
    .fetch_all(executor)
    .await
}

pub async fn insert<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
    pattern: &Pattern,
) -> Result<u64, sqlx::Error> {
    let surt_string = pattern.surt.to_string();
    let sort_id = pattern.sort_id as i64;
    let id = query_scalar!(
        "INSERT INTO pattern(surt, prefix, name, slug, sort_id)
            VALUES (?, ?, ?, ?, ?) ON CONFLICT DO UPDATE SET id = id RETURNING id",
        surt_string,
        pattern.prefix,
        pattern.name,
        pattern.slug,
        sort_id
    )
    .persistent(true)
    .fetch_one(executor)
    .await?;

    Ok(id as u64)
}

pub async fn insert_pattern_entry<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
    pattern_id: u64,
    entry_id: u64,
) -> Result<(), sqlx::Error> {
    let pattern_id = pattern_id as i64;
    let entry_id = entry_id as i64;

    query!(
        "INSERT OR IGNORE INTO pattern_entry(pattern_id, entry_id) VALUES (?, ?)",
        pattern_id,
        entry_id
    )
    .persistent(true)
    .execute(executor)
    .await?;

    Ok(())
}
