use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::ops::Bound;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Range<A> {
    Start(A),
    End(A),
    Both(A, A),
}

impl<A> Range<A> {
    pub fn new(start: Option<A>, end: Option<A>) -> Option<Self> {
        match (start, end) {
            (None, None) => None,
            (Some(start), None) => Some(Self::Start(start)),
            (None, Some(end)) => Some(Self::End(end)),
            (Some(start), Some(end)) => Some(Self::Both(start, end)),
        }
    }

    pub fn start(&self) -> Option<&A> {
        match self {
            Self::Start(start) | Self::Both(start, _) => Some(start),
            Self::End(_) => None,
        }
    }

    pub fn end(&self) -> Option<&A> {
        match self {
            Self::End(end) | Self::Both(_, end) => Some(end),
            Self::Start(_) => None,
        }
    }

    pub fn map<B, F: Fn(&A) -> B>(&self, f: F) -> Range<B> {
        match self {
            Self::Start(start) => Range::Start(f(start)),
            Self::End(end) => Range::End(f(end)),
            Self::Both(start, end) => Range::Both(f(start), f(end)),
        }
    }

    pub fn bounds<MIN: Fn() -> A, MAX: Fn() -> A>(
        self,
        min: MIN,
        max: MAX,
    ) -> (Bound<A>, Bound<A>) {
        match self {
            Self::Start(start) => (Bound::Included(start), Bound::Excluded(max())),
            Self::End(end) => (Bound::Included(min()), Bound::Excluded(end)),
            Self::Both(start, end) => (Bound::Included(start), Bound::Excluded(end)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Query {
    pub content: String,
    pub gravatar_hash: Option<String>,
    pub date_range: Option<Range<DateTime<Utc>>>,
    pub pattern_slugs: Option<HashSet<String>>,
    pub years: Option<HashSet<u16>>,
}

impl Query {
    pub fn new(
        content: &str,
        gravatar_email: Option<&str>,
        date_range: Option<Range<DateTime<Utc>>>,
        pattern_slugs: Vec<String>,
        years: Vec<u16>,
    ) -> Self {
        let pattern_slugs = if pattern_slugs.is_empty() {
            None
        } else {
            Some(pattern_slugs.into_iter().collect())
        };

        let years = if years.is_empty() {
            None
        } else {
            Some(years.into_iter().collect())
        };

        Self {
            content: content.to_string(),
            gravatar_hash: gravatar_email.map(Self::hash_email),
            date_range,
            pattern_slugs,
            years,
        }
    }

    fn hash_email(email: &str) -> String {
        format!("{:x}", md5::compute(email.to_ascii_lowercase()))
    }
}
