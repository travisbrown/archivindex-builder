use crate::{digest::Digest, timestamp::Timestamp};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

const WAYBACK_URL_PATTERN: &str =
    r"^http(:?s)?://web.archive.org/web/(?P<timestamp>\d{14})(?:id_)?/(?P<url>.+)$";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid URL")]
    InvalidUrl(String),
    #[error("Invalid timestamp")]
    InvalidTimestamp(#[from] crate::timestamp::Error),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct UrlParts {
    pub url: String,
    pub timestamp: Timestamp,
}

impl UrlParts {
    pub fn new(url: String, timestamp: Timestamp) -> Self {
        Self { url, timestamp }
    }

    pub fn to_wb_url(&self, https: bool, original: bool) -> String {
        format!(
            "http{}://web.archive.org/web/{}{}/{}",
            if https { "s" } else { "" },
            self.timestamp,
            if original { "id_" } else { "" },
            self.url
        )
    }
}

impl FromStr for UrlParts {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        static WAYBACK_URL_RE: Lazy<regex::Regex> =
            Lazy::new(|| regex::Regex::new(WAYBACK_URL_PATTERN).unwrap());

        let captures = WAYBACK_URL_RE
            .captures(s)
            .ok_or(Error::InvalidUrl(s.to_string()))?;

        Ok(Self::new(
            captures["url"].to_string(),
            captures["timestamp"].to_string().parse()?,
        ))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct EntryInfo {
    pub url_parts: UrlParts,
    pub expected_digest: Digest,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let url = "https://web.archive.org/web/20160508215503/https://twitter.com/roman_dmowski99/status/725877225686454272";
        let expected = UrlParts::new(
            "https://twitter.com/roman_dmowski99/status/725877225686454272".to_string(),
            "20160508215503".parse().unwrap(),
        );

        let parsed: UrlParts = url.parse().unwrap();

        assert_eq!(parsed, expected);
    }
}
