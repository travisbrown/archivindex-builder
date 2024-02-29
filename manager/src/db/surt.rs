use sqlx::{query_scalar, Executor, Sqlite};

pub async fn insert<'c, E: Executor<'c, Database = Sqlite>>(
    executor: E,
    value: &str,
) -> Result<u64, sqlx::Error> {
    let id = query_scalar!(
        "INSERT INTO surt(value)
            VALUES (?) ON CONFLICT DO UPDATE SET id = id RETURNING id",
        value
    )
    .persistent(true)
    .fetch_one(executor)
    .await?;

    Ok(id as u64)
}
