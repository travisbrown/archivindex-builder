use aib_extractor::Document;
use chrono::{DateTime, Datelike, Utc};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tantivy::{
    collector::{FacetCollector, FacetCounts},
    directory::MmapDirectory,
    doc,
    query::{BooleanQuery, Occur, QueryParser, RangeQuery, TermQuery, TermSetQuery},
    schema::{Facet, IndexRecordOption, Term},
    DocAddress, IndexReader, IndexWriter, SnippetGenerator,
};

pub mod collector;
pub mod query;
pub mod schema;
pub mod snippet;

use collector::TopDocs;

pub use query::Query;
pub use snippet::Snippet;

const WRITER_BUFFER_SIZE: usize = 100_000_000;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("Tantivy error")]
    Tantivy(#[from] tantivy::error::TantivyError),
    #[error("Tantivy query error")]
    TantivyQuery(#[from] tantivy::query::QueryParserError),
    #[error("Tantivy directory error")]
    TantivyDirectory(#[from] tantivy::directory::error::OpenDirectoryError),
    #[error("Missing snapshot ID")]
    MissingSnapshotId(DocAddress),
    #[error("Missing SURT ID")]
    MissingSurtId(DocAddress),
    #[error("Unexpected SURT ID")]
    UnexpectedSurtId(DocAddress),
    #[error("Missing pattern")]
    MissingPattern(DocAddress),
    #[error("Missing title")]
    MissingTitle(DocAddress),
}

#[derive(Debug)]
pub struct SearchResults {
    pub pattern_counts: IndexMap<String, usize>,
    pub year_counts: IndexMap<u16, usize>,
    pub hits: Vec<(u64, Vec<SearchHit>)>,
}

#[derive(Debug)]
pub struct SearchHit {
    pub score: f32,
    pub snapshot_id: i64,
    pub pattern_slug: String,
    pub address: DocAddress,
    pub title: String,
    pub snippet: Snippet,
}

pub struct Index {
    schema: schema::Schema,
    surt_ids: Option<Arc<HashMap<DocAddress, u64>>>,
    writer: IndexWriter,
    reader: IndexReader,
    query_parser: QueryParser,
    pattern_slugs: Vec<String>,
    years: Vec<u16>,
}

impl Index {
    pub fn open<P: AsRef<Path>>(
        path: P,
        pattern_slugs: &[&str],
        first_year: u16,
    ) -> Result<Self, Error> {
        let schema = schema::Schema::default();
        let index =
            tantivy::Index::open_or_create(MmapDirectory::open(path)?, schema.schema.clone())?;
        let writer = index.writer(WRITER_BUFFER_SIZE)?;
        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::OnCommit)
            .try_into()?;
        let query_parser =
            QueryParser::for_index(&index, vec![schema.fields.title, schema.fields.content]);

        let pattern_slugs = pattern_slugs
            .iter()
            .map(|pattern_slug| pattern_slug.to_string())
            .collect();

        let mut years: Vec<u16> = (first_year..=Utc::now().year() as u16).collect();
        years.reverse();

        Ok(Self {
            schema,
            surt_ids: None,
            writer,
            reader,
            query_parser,
            pattern_slugs,
            years,
        })
    }

    pub fn initialize_surt_ids(&mut self) -> Result<usize, Error> {
        let mut surt_ids = HashMap::new();
        let searcher = self.reader.searcher();

        for (segment_ord, segment_reader) in searcher.segment_readers().iter().enumerate() {
            for doc_id in segment_reader.doc_ids_alive() {
                let doc_address = DocAddress::new(segment_ord as u32, doc_id);
                let doc = searcher.doc(doc_address)?;
                let surt_id = doc
                    .get_first(self.schema.fields.surt_id)
                    .and_then(|field| field.as_i64())
                    .map(|value| value as u64)
                    .ok_or_else(|| Error::MissingSurtId(doc_address))?;
                surt_ids.insert(doc_address, surt_id);
            }
        }

        let count = surt_ids.len();
        self.surt_ids = Some(Arc::new(surt_ids));

        Ok(count)
    }

    fn pattern_facet_collector(&self) -> FacetCollector {
        let mut collector = FacetCollector::for_field(schema::PATTERN_FIELD_NAME);
        collector.add_facet(Facet::from("/"));
        collector
    }

    fn year_facet_collector(&self) -> FacetCollector {
        let mut collector = FacetCollector::for_field(schema::YEAR_FIELD_NAME);
        collector.add_facet(Facet::from("/"));
        collector
    }

    fn pattern_facet_counts(&self, facet_counts: &FacetCounts) -> IndexMap<String, usize> {
        let mut counts = IndexMap::new();

        for pattern in &self.pattern_slugs {
            counts.insert(pattern.clone(), 0);
        }

        for (facet, count) in facet_counts.get("/") {
            let mut pattern = facet.to_string();
            pattern.remove(0);

            counts.insert(pattern, count as usize);
        }

        counts
    }

    fn year_facet_counts(&self, facet_counts: &FacetCounts) -> IndexMap<u16, usize> {
        let mut counts = IndexMap::new();

        for year in &self.years {
            counts.insert(*year, 0);
        }

        for (facet, count) in facet_counts.get("/") {
            let year = facet.to_string()[1..].parse::<u16>().unwrap_or(0);

            counts.insert(year, count as usize);
        }

        counts
    }

    pub fn add_document(
        &mut self,
        snapshot_id: i64,
        surt_id: i64,
        pattern_slug: &str,
        timestamp: DateTime<Utc>,
        document: &Document,
    ) -> Result<(), Error> {
        let mut gravatar_hashes = document.gravatar_hashes.iter().cloned().collect::<Vec<_>>();
        gravatar_hashes.sort();

        let document = doc!(
            self.schema.fields.snapshot_id => snapshot_id,
            self.schema.fields.surt_id => surt_id,
            self.schema.fields.pattern => Facet::from(&format!("/{}", pattern_slug)),
            self.schema.fields.year => Facet::from(&format!("/{}", timestamp.year())),
            self.schema.fields.timestamp => Self::to_tantivy_date_time(timestamp),
            self.schema.fields.title => document.title.to_string(),
            self.schema.fields.content => document.content.join(" "),
            self.schema.fields.gravatar_hashes => gravatar_hashes.join(" ")
        );

        self.writer.add_document(document)?;

        Ok(())
    }

    pub fn commit_writer(&mut self) -> Result<(), Error> {
        self.writer.commit()?;

        Ok(())
    }

    pub fn search(
        &self,
        snippet_max_chars: usize,
        query: &Query,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResults, Error> {
        let query = self.to_tantivy_query(query)?;
        let searcher = self.reader.searcher();
        let mut snippet_generator =
            SnippetGenerator::create(&searcher, &*query, self.schema.fields.content)?;
        snippet_generator.set_max_num_chars(snippet_max_chars);

        let collector = (
            (self.pattern_facet_collector(), self.year_facet_collector()),
            TopDocs::new(limit, offset, self.surt_ids.as_ref().unwrap().clone()),
        );

        let ((pattern_facet_counts, year_facet_counts), results) =
            searcher.search(&query, &collector)?;
        let pattern_counts = self.pattern_facet_counts(&pattern_facet_counts);
        let year_counts = self.year_facet_counts(&year_facet_counts);

        let results = results
            .top()
            .into_iter()
            .map(|(_score, (surt_id, docs))| {
                let hits = docs
                    .into_iter()
                    .map(|(score, address)| {
                        let retrieved_document = searcher.doc(address)?;
                        let snippet = snippet_generator.snippet_from_doc(&retrieved_document);

                        let snapshot_id = retrieved_document
                            .get_first(self.schema.fields.snapshot_id)
                            .and_then(|field| field.as_i64())
                            .ok_or_else(|| Error::MissingSnapshotId(address))?;
                        let retrieved_surt_id = retrieved_document
                            .get_first(self.schema.fields.surt_id)
                            .and_then(|field| field.as_i64())
                            .ok_or_else(|| Error::MissingSurtId(address))?;

                        if retrieved_surt_id as u64 != surt_id {
                            Err(Error::UnexpectedSurtId(address))
                        } else {
                            let pattern = retrieved_document
                                .get_first(self.schema.fields.pattern)
                                .and_then(|field| field.as_facet())
                                .ok_or_else(|| Error::MissingPattern(address))?;
                            let title = retrieved_document
                                .get_first(self.schema.fields.title)
                                .and_then(|field| field.as_text())
                                .ok_or_else(|| Error::MissingTitle(address))?
                                .to_string();
                            Ok(SearchHit {
                                score,
                                snapshot_id,
                                pattern_slug: pattern.to_string(),
                                address,
                                title,
                                snippet: (&snippet).into(),
                            })
                        }
                    })
                    .collect::<Result<Vec<_>, Error>>()?;

                Ok((surt_id, hits))
            })
            .collect::<Result<_, Error>>()?;

        Ok(SearchResults {
            pattern_counts,
            year_counts,
            hits: results,
        })
    }

    pub fn to_tantivy_query(&self, query: &Query) -> Result<Box<dyn tantivy::query::Query>, Error> {
        let content_query = self.query_parser.parse_query(&query.content)?;

        let gravatar_hash_query = query.gravatar_hash.as_ref().map(|gravatar_hash| {
            TermQuery::new(
                Term::from_field_text(self.schema.fields.gravatar_hashes, gravatar_hash),
                IndexRecordOption::Basic,
            )
        });

        let date_range_query = query.date_range.as_ref().map(|date_range| {
            let terms = date_range.map(|value| {
                Term::from_field_date(
                    self.schema.fields.timestamp,
                    Self::to_tantivy_date_time(*value),
                )
            });

            let (lower_bound, upper_bound) = terms.bounds(
                || Term::from_field_date(self.schema.fields.timestamp, tantivy::DateTime::MIN),
                || Term::from_field_date(self.schema.fields.timestamp, tantivy::DateTime::MAX),
            );

            RangeQuery::new_term_bounds(
                schema::TIMESTAMP_FIELD_NAME.to_string(),
                tantivy::schema::Type::Date,
                &lower_bound,
                &upper_bound,
            )
        });

        let pattern_query = query.pattern_slugs.as_ref().map(|pattern_slugs| {
            let terms = pattern_slugs
                .iter()
                .map(|pattern_slug| {
                    Term::from_facet(
                        self.schema.fields.pattern,
                        &Facet::from(&format!("/{}", pattern_slug)),
                    )
                })
                .collect::<Vec<_>>();

            TermSetQuery::new(terms)
        });

        let year_query = query.years.as_ref().map(|years| {
            let terms = years
                .iter()
                .map(|year| {
                    Term::from_facet(self.schema.fields.year, &Facet::from(&format!("/{}", year)))
                })
                .collect::<Vec<_>>();

            TermSetQuery::new(terms)
        });

        if gravatar_hash_query.is_none()
            && date_range_query.is_none()
            && pattern_query.is_none()
            && year_query.is_none()
        {
            Ok(content_query)
        } else {
            let mut parts = vec![(Occur::Must, content_query)];

            if let Some(query) = gravatar_hash_query {
                parts.push((Occur::Must, Box::new(query)));
            }

            if let Some(query) = date_range_query {
                parts.push((Occur::Must, Box::new(query)));
            }

            if let Some(query) = pattern_query {
                parts.push((Occur::Must, Box::new(query)));
            }

            if let Some(query) = year_query {
                parts.push((Occur::Must, Box::new(query)));
            }

            Ok(Box::new(BooleanQuery::new(parts)))
        }
    }

    fn to_tantivy_date_time(value: DateTime<Utc>) -> tantivy::DateTime {
        tantivy::DateTime::from_timestamp_secs(value.timestamp())
    }
}
