use aib_extractor::Document;
use aib_indexer::{Index, Query};
use itertools::Itertools;
use sqlx::SqlitePool;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub mod db;
pub mod import;
pub mod model;
pub mod search;

const DEFAULT_FIRST_YEAR: u16 = 2004;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("I/O error")]
    IoWithPath(std::io::Error, PathBuf),
    #[error("SQL error")]
    Sqlx(#[from] sqlx::Error),
    #[error("Digest error")]
    Digest(#[from] aib_core::digest::Error),
    #[error("SURT error")]
    Surt(#[from] aib_core::surt::Error),
    #[error("DB error")]
    Db(#[from] db::Error),
    #[error("CDX store error")]
    CdxStore(#[from] aib_cdx_store::Error),
    #[error("Item store error")]
    Store(#[from] aib_store::items::Error),
    #[error("Downloader error")]
    Downloader(#[from] aib_downloader::Error),
    #[error("Extractor error")]
    Extractor(#[from] aib_extractor::Error),
    #[error("Index error")]
    Index(#[from] aib_indexer::Error),
    #[error("Search error")]
    Search(#[from] search::Error),
    #[error("Snapshot missing for digest")]
    MissingSnapshot(String),
}

pub struct Manager {
    db_pool: SqlitePool,
    pub index: Index,
    store: aib_store::items::ItemStore,
}

impl Manager {
    pub async fn open<P: AsRef<Path>>(
        db_url: &str,
        index_path: P,
        store_path: P,
        level: Option<i32>,
    ) -> Result<Self, Error> {
        let pool = SqlitePool::connect(db_url).await?;
        let patterns = db::pattern::get_all(&mut *pool.acquire().await?).await?;
        let pattern_slugs = patterns
            .iter()
            .map(|pattern| pattern.slug.as_str())
            .collect::<Vec<_>>();

        Ok(Self {
            db_pool: pool,
            index: Index::open(index_path, &pattern_slugs, DEFAULT_FIRST_YEAR)?,
            store: aib_store::items::ItemStore::new(store_path, level),
        })
    }

    pub fn extract(&self) -> Result<(), Error> {
        let mut buffer = String::new();
        for path in self.store.files() {
            let path = path?;
            let mut decoder = zstd::Decoder::new(File::open(&path)?)?;
            buffer.clear();
            match decoder
                .read_to_string(&mut buffer)
                .map_err(|error| Error::IoWithPath(error, path))
            {
                Ok(_) => {
                    let html = Document::parse(&buffer)?;

                    for link in html.links {
                        println!("{}", link);
                    }
                }
                Err(error) => {
                    log::warn!("{:?}", error);
                }
            }
        }

        Ok(())
    }

    pub async fn index(&mut self, mime_type: &str) -> Result<usize, Error> {
        let mut connection = self.db_pool.acquire().await?;
        let mut db = db::Db::new(&mut connection);

        let snapshot_info = db.get_snapshot_info(mime_type).await?;
        let mut buffer = String::new();
        let mut count = 0;

        for (_, mut group) in &snapshot_info
            .into_iter()
            .group_by(|(snapshot_id, _, _, _, _)| *snapshot_id)
        {
            // Safe because of guarantees provided by Itertools.
            let (snapshot_id, surt_id, pattern_slug, digest, timestamp) = group.next().unwrap();

            let path = self
                .store
                .location(&digest)
                .ok_or_else(|| Error::MissingSnapshot(digest))?;

            let mut decoder = zstd::Decoder::new(File::open(&path)?)?;
            buffer.clear();

            match decoder
                .read_to_string(&mut buffer)
                .map_err(|error| Error::IoWithPath(error, path))
            {
                Ok(_) => {
                    let html = scraper::Html::parse_document(&buffer);
                    let document = Document::extract(&html)?;

                    self.index.add_document(
                        snapshot_id,
                        surt_id,
                        &pattern_slug,
                        timestamp,
                        &document,
                    )?;

                    count += 1;
                }
                Err(error) => {
                    log::warn!("{:?}", error);
                }
            }
        }

        self.index.commit_writer()?;

        Ok(count)
    }

    pub async fn search(
        &self,
        snippet_max_chars: usize,
        query: &Query,
        limit: usize,
        offset: usize,
    ) -> Result<search::SearchResult, Error> {
        let mut connection = self.db_pool.acquire().await?;
        let db = db::Db::new(&mut connection);

        Ok(crate::search::search(&self.index, db, snippet_max_chars, query, limit, offset).await?)
    }
}
