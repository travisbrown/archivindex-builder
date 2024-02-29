use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{
    de::{Deserialize, Deserializer, Unexpected, Visitor},
    ser::{Serialize, Serializer},
};
use std::fmt::Display;
use std::str::FromStr;

const TIMESTAMP_FMT: &str = "%Y%m%d%H%M%S";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid timestamp length")]
    InvalidLength(String),
    #[error("Invalid timestamp input")]
    InvalidDateTime(#[from] chrono::format::ParseError),
    #[error("Invalid timestamp")]
    InvalidTimestamp(i64),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(pub DateTime<Utc>);

impl Timestamp {}

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.format(TIMESTAMP_FMT))
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(value: DateTime<Utc>) -> Self {
        Self(value)
    }
}

impl TryFrom<i64> for Timestamp {
    type Error = Error;
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(Self(
            DateTime::from_timestamp(value, 0).ok_or(Error::InvalidTimestamp(value))?,
        ))
    }
}

impl FromStr for Timestamp {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 14 {
            Ok(Timestamp(
                NaiveDateTime::parse_from_str(s, TIMESTAMP_FMT)?.and_utc(),
            ))
        } else {
            Err(Self::Err::InvalidLength(s.to_string()))
        }
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TimestampVisitor;

        impl<'de> Visitor<'de> for TimestampVisitor {
            type Value = Timestamp;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Timestamp")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                v.parse()
                    .map_err(|_| serde::de::Error::invalid_value(Unexpected::Str(v), &self))
            }
        }

        deserializer.deserialize_str(TimestampVisitor)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{SubsecRound, Utc};

    #[test]
    fn round_trip() {
        let timestamp = super::Timestamp(Utc::now().trunc_subsecs(0));

        let timestamp_str = timestamp.to_string();
        let timestamp_parsed = timestamp_str.parse().unwrap();

        assert_eq!(timestamp, timestamp_parsed);
    }
}
