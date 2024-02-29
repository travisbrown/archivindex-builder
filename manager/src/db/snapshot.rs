use sqlx::{query_scalar, Executor, Sqlite};

pub async fn insert<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
    digest: &str,
) -> Result<i64, sqlx::Error> {
    query_scalar!(
        "INSERT INTO snapshot(digest)
            VALUES (?) ON CONFLICT DO UPDATE SET id = id RETURNING id",
        digest
    )
    .persistent(true)
    .fetch_one(executor)
    .await
}
