use aib_auth::AuthDb;
use egg_mode::{KeyPair, Token};
use sqlx::{Row, SqliteConnection};

pub struct SqlxAuthDb;

#[async_trait::async_trait]
impl AuthDb for SqlxAuthDb {
    type Connection = SqliteConnection;
    type Error = Error;

    async fn get_github_name(
        connection: &mut Self::Connection,
        id: u64,
    ) -> Result<Option<String>, Self::Error> {
        let id = u64_to_i64(id)?;
        Ok(
            sqlx::query_scalar("SELECT value FROM github_names WHERE id = ?")
                .bind(id)
                .persistent(true)
                .fetch_optional(connection)
                .await?,
        )
    }

    async fn get_google_email(
        connection: &mut Self::Connection,
        sub: &str,
    ) -> Result<Option<String>, Self::Error> {
        Ok(
            sqlx::query_scalar("SELECT value FROM google_names WHERE id = ?")
                .bind(sub)
                .persistent(true)
                .fetch_optional(connection)
                .await?,
        )
    }

    async fn get_google_sub(
        connection: &mut Self::Connection,
        email: &str,
    ) -> Result<Option<String>, Self::Error> {
        Ok(
            sqlx::query_scalar("SELECT id FROM google_names WHERE value = ?")
                .bind(email)
                .persistent(true)
                .fetch_optional(connection)
                .await?,
        )
    }

    async fn get_twitter_name(
        connection: &mut Self::Connection,
        id: u64,
    ) -> Result<Option<String>, Self::Error> {
        let id = u64_to_i64(id)?;
        Ok(
            sqlx::query_scalar("SELECT value FROM twitter_names WHERE id = ?")
                .bind(id)
                .persistent(true)
                .fetch_optional(connection)
                .await?,
        )
    }

    async fn put_github_name(
        connection: &mut Self::Connection,
        id: u64,
        value: &str,
    ) -> Result<(), Self::Error> {
        let id = u64_to_i64(id)?;
        sqlx::query("REPLACE INTO github_names (id, value) VALUES (?, ?)")
            .bind(id)
            .bind(value)
            .persistent(true)
            .execute(connection)
            .await?;

        Ok(())
    }

    async fn put_google_email(
        connection: &mut Self::Connection,
        sub: &str,
        value: &str,
    ) -> Result<(), Self::Error> {
        sqlx::query("REPLACE INTO google_names (id, value) VALUES (?, ?)")
            .bind(sub)
            .bind(value)
            .persistent(true)
            .execute(connection)
            .await?;

        Ok(())
    }

    async fn put_twitter_name(
        connection: &mut Self::Connection,
        id: u64,
        value: &str,
    ) -> Result<(), Self::Error> {
        let id = u64_to_i64(id)?;
        sqlx::query("REPLACE INTO twitter_names (id, value) VALUES (?, ?)")
            .bind(id)
            .bind(value)
            .persistent(true)
            .execute(connection)
            .await?;

        Ok(())
    }

    async fn lookup_github_token(
        connection: &mut Self::Connection,
        token: &str,
    ) -> Result<Option<(u64, bool)>, Self::Error> {
        Ok(
            sqlx::query("SELECT id, gist FROM github_tokens WHERE value = ?")
                .bind(token)
                .persistent(true)
                .fetch_optional(connection)
                .await?,
        )
        .map(|result| {
            result.map(|row| (row.get::<i64, _>("id") as u64, row.get::<bool, _>("gist")))
        })
    }

    async fn lookup_google_token(
        connection: &mut Self::Connection,
        token: &str,
    ) -> Result<Option<(String, String)>, Self::Error> {
        Ok(sqlx::query(
            "SELECT google_tokens.id AS sub, google_names.value AS email
                FROM google_tokens
                JOIN google_names ON google_names.id = google_tokens.id
                WHERE google_tokens.value = ?",
        )
        .bind(token)
        .persistent(true)
        .fetch_optional(connection)
        .await?)
        .map(|result| {
            result.map(|row| (row.get::<String, _>("sub"), row.get::<String, _>("email")))
        })
    }

    async fn lookup_twitter_token(
        connection: &mut Self::Connection,
        token: &str,
    ) -> Result<Option<u64>, Self::Error> {
        Ok(
            sqlx::query_scalar::<_, i64>("SELECT id FROM twitter_tokens WHERE value = ?")
                .bind(token)
                .persistent(true)
                .fetch_optional(connection)
                .await?,
        )
        .map(|result| result.map(|id| id as u64))
    }

    async fn get_twitter_access_token(
        connection: &mut Self::Connection,
        token: &str,
    ) -> Result<Option<Token>, Self::Error> {
        Ok(sqlx::query(
            "SELECT id, consumer_secret, access_key, access_secret
                    FROM twitter_tokens
                    WHERE value = ?",
        )
        .bind(token)
        .persistent(true)
        .fetch_optional(connection)
        .await?)
        .map(|result| {
            result.map(|row| Token::Access {
                consumer: KeyPair::new(token.to_string(), row.get::<String, _>("consumer_secret")),
                access: KeyPair::new(
                    row.get::<String, _>("access_key"),
                    row.get::<String, _>("access_secret"),
                ),
            })
        })
    }

    async fn put_github_token(
        connection: &mut Self::Connection,
        token: &str,
        id: u64,
        gist: bool,
    ) -> Result<(), Self::Error> {
        let id = u64_to_i64(id)?;
        sqlx::query("INSERT INTO github_tokens (value, id, gist) VALUES (?, ?, ?)")
            .bind(token)
            .bind(id)
            .bind(gist)
            .persistent(true)
            .execute(connection)
            .await?;

        Ok(())
    }

    async fn put_google_token(
        connection: &mut Self::Connection,
        token: &str,
        sub: &str,
    ) -> Result<(), Self::Error> {
        sqlx::query("INSERT INTO google_tokens (value, id) VALUES (?, ?)")
            .bind(token)
            .bind(sub)
            .persistent(true)
            .execute(connection)
            .await?;

        Ok(())
    }

    async fn put_twitter_token(
        connection: &mut Self::Connection,
        token: &str,
        id: u64,
        consumer_secret: &str,
        access_key: &str,
        access_secret: &str,
    ) -> Result<(), Self::Error> {
        let id = u64_to_i64(id)?;
        sqlx::query(
            "INSERT INTO twitter_tokens (value, id, consumer_secret, access_key, access_secret)
                VALUES (?, ?, ?, ?, ?)",
        )
        .bind(token)
        .bind(id)
        .bind(consumer_secret)
        .bind(access_key)
        .bind(access_secret)
        .persistent(true)
        .execute(connection)
        .await?;

        Ok(())
    }
}

fn u64_to_i64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::InvalidId(value))
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("SQLx error")]
    Sqlx(#[from] sqlx::Error),
    #[error("Invalid ID")]
    InvalidId(u64),
}
