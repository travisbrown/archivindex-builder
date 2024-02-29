use aib_core::surt::Surt;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use std::collections::HashMap;

const SNIPPET_HIGHLIGHT_TAG: &str = "strong";

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct SearchResult {
    patterns: IndexMap<String, usize>,
    years: IndexMap<u16, usize>,
    pages: Vec<PageResult>,
}

impl From<aib_manager::search::SearchResult> for SearchResult {
    fn from(value: aib_manager::search::SearchResult) -> Self {
        //let mut pages = Vec::with_capacity(value.surts.len());
        //let mut title_counts = HashMap::new();

        let pages = value
            .surts
            .into_iter()
            .map(|(surt, hits)| {
                let mut snapshots = Vec::with_capacity(hits.len());
                let mut scores = IndexMap::new();
                let mut title_counts = HashMap::<_, usize>::new();

                for (timestamp, hit) in hits {
                    let score = hit.as_ref().map(|value| value.score);

                    if let Some(hit) = hit {
                        let title_entry = title_counts.entry(hit.title.clone()).or_default();
                        *title_entry += 1;

                        snapshots.push(SnapshotResult {
                            timestamp: timestamp.0,
                            score: hit.score,
                            url: hit.url.to_wb_url(true, false),
                            title: hit.title,
                            snippet: hit.snippet.to_html(SNIPPET_HIGHLIGHT_TAG),
                        });
                    }

                    scores.insert(timestamp.0.timestamp() as u64, score);
                }

                let title = title_counts
                    .iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(title, _)| title)
                    .cloned()
                    .unwrap_or_default();

                let url = surt.canonical_url().to_string();

                snapshots.reverse();

                PageResult {
                    surt,
                    url,
                    title,
                    snapshots,
                    scores,
                }
            })
            .collect();

        Self {
            patterns: value.pattern_counts,
            years: value.year_counts,
            pages,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PageResult {
    pub surt: Surt,
    pub url: String,
    pub title: String,
    pub snapshots: Vec<SnapshotResult>,
    pub scores: IndexMap<u64, Option<f32>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct SnapshotResult {
    pub timestamp: DateTime<Utc>,
    pub score: f32,
    pub url: String,
    pub title: String,
    pub snippet: String,
}
