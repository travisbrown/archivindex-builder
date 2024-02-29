use aib_core::{entry::UrlParts, surt::Surt, timestamp::Timestamp};
use aib_indexer::{Index, Query, Snippet};
use indexmap::IndexMap;
use itertools::Itertools;
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::collections::HashMap;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Index error")]
    Index(#[from] aib_indexer::Error),
    #[error("DB error")]
    Db(#[from] crate::db::Error),
    #[error("Snapshot missing for ID")]
    MissingSnapshot(i64),
    #[error("Pattern missing")]
    MissingPattern(i64),
    #[error("SURT timestamps missing")]
    MissingSurtTimestamps(i64),
    #[error("Invalid SURT")]
    Surt(#[from] aib_core::surt::Error),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct SearchResult {
    pub pattern_counts: IndexMap<String, usize>,
    pub year_counts: IndexMap<u16, usize>,
    pub surts: IndexMap<Surt, IndexMap<Timestamp, Option<Hit>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub score: f32,
    pub pattern_slug: String,
    pub url: UrlParts,
    pub title: String,
    pub snippet: Snippet,
}

impl Serialize for Hit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut result = serializer.serialize_struct("Hit", 5)?;
        result.serialize_field("url", &self.url.to_wb_url(true, false))?;
        result.serialize_field("score", &self.score)?;
        result.serialize_field("pattern", &self.pattern_slug)?;
        result.serialize_field("title", &self.title)?;
        result.serialize_field("snippet", &self.snippet)?;
        result.end()
    }
}

pub async fn search<'a>(
    index: &Index,
    mut db: crate::db::Db<'a>,
    snippet_max_chars: usize,
    query: &Query,
    limit: usize,
    offset: usize,
) -> Result<SearchResult, Error> {
    let results = index.search(snippet_max_chars, query, limit, offset)?;

    let mut snapshot_ids = vec![];
    let mut snapshot_map = HashMap::new();

    for (_surt_id, hits) in results.hits {
        for hit in hits {
            snapshot_ids.push(hit.snapshot_id);
            snapshot_map.insert(
                hit.snapshot_id,
                (hit.pattern_slug, hit.score, hit.title, hit.snippet),
            );
        }
    }

    let (snapshots, surt_entries) = db
        .get_search_result(&query.date_range, &snapshot_ids)
        .await?;

    let mut surts = IndexMap::new();

    for (surt, group) in &snapshots.into_iter().group_by(|(_, _, surt)| surt.clone()) {
        let mut surt_timestamps = surt_entries
            .get(&surt.id)
            .ok_or_else(|| Error::MissingSurtTimestamps(surt.id))?
            .clone();
        surt_timestamps.sort();

        let mut surt_results = surt_timestamps
            .into_iter()
            .map(|timestamp| (timestamp, None))
            .collect::<IndexMap<_, _>>();

        for (snapshot_id, url, _surt) in group {
            let (pattern_slug, score, title, snippet) = snapshot_map
                .get(&snapshot_id)
                .cloned()
                .ok_or_else(|| Error::MissingSnapshot(snapshot_id))?;

            // TODO: Confirm that this timestamp was in our list?
            surt_results.insert(
                url.timestamp,
                Some(Hit {
                    score,
                    pattern_slug,
                    url,
                    title,
                    snippet,
                }),
            );
        }
        let surt = surt.value.parse::<Surt>()?;

        surts.insert(surt, surt_results);
    }

    Ok(SearchResult {
        pattern_counts: results.pattern_counts,
        year_counts: results.year_counts,
        surts,
    })
}
