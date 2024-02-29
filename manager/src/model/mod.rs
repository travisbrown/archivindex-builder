pub mod entry;
pub mod pattern;

pub use entry::Entry;
pub use pattern::Pattern;

fn try_cast<S, T>(value: S) -> Result<T, sqlx::Error>
where
    S: TryInto<T>,
    <S as TryInto<T>>::Error: std::error::Error + Send + Sync + 'static,
{
    value
        .try_into()
        .map_err(|error| sqlx::Error::Decode(Box::new(error)))
}
