use aib_cdx::entry::{Entry, EntryList};
use chrono::{DateTime, Utc};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("Entry error")]
    Entry(#[from] aib_cdx::client::Error),
    #[error("JSON error")]
    Json(serde_json::Error, PathBuf),
    #[error("Invalid page path")]
    InvalidPagePath(PathBuf),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EntryPage {
    timestamp: DateTime<Utc>,
    url: String,
    content: String,
}

impl EntryPage {
    pub fn new(page: &aib_cdx::client::Page) -> Self {
        Self {
            timestamp: Utc::now(),
            url: page.url.clone(),
            content: page.content.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Store {
    base: PathBuf,
    compression_level: Option<i32>,
    query_dir: PathBuf,
    data_dir: PathBuf,
}

impl Store {
    pub fn new<P: AsRef<Path>>(base: P, compression_level: Option<i32>) -> Self {
        let base = base.as_ref().to_path_buf();
        let query_dir = base.join("queries");
        let data_dir = base.join("data");
        Self {
            compression_level,
            base,
            query_dir,
            data_dir,
        }
    }

    fn init(&self) -> Result<(), Error> {
        std::fs::create_dir_all(&self.base)?;
        std::fs::create_dir_all(&self.query_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;

        Ok(())
    }

    pub fn entries(&self) -> Result<Vec<(DateTime<Utc>, Entry)>, Error> {
        let mut data_files = std::fs::read_dir(&self.data_dir)?
            .map(|page_entry| {
                let page_path = page_entry?.path();
                let file_name = page_path
                    .file_name()
                    .and_then(|file_name| file_name.to_str())
                    .ok_or_else(|| Error::InvalidPagePath(page_path.clone()))?;

                if (self.compression_level.is_none() && !file_name.ends_with(".json"))
                    || self.compression_level.is_some() && !file_name.ends_with(".json.zst")
                {
                    Err(Error::InvalidPagePath(page_path.clone()))
                } else {
                    let timestamp_ms = file_name
                        .split('.')
                        .next()
                        .and_then(|first_part| first_part.parse::<i64>().ok())
                        .and_then(DateTime::from_timestamp_millis)
                        .ok_or_else(|| Error::InvalidPagePath(page_path.clone()))?;

                    Ok((timestamp_ms, page_path))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        data_files.sort();

        let mut results = vec![];

        for (timestamp, path) in data_files {
            let file = File::open(&path)?;
            let reader: Box<dyn Read> = if self.compression_level.is_some() {
                Box::new(zstd::Decoder::new(file)?)
            } else {
                Box::new(BufReader::new(file))
            };

            let entry_list = serde_json::from_reader::<_, EntryList>(reader)
                .map_err(|error| Error::Json(error, path.clone()))?;

            for entry in entry_list.values {
                results.push((timestamp, entry));
            }
        }

        Ok(results)
    }

    pub fn add_entry_pages(&self, entry_pages: &[EntryPage]) -> Result<usize, Error> {
        self.init()?;

        let mut pages = entry_pages.iter().collect::<Vec<_>>();
        pages.sort();

        if let Some(last_page) = pages.last() {
            let timestamp_ms = last_page.timestamp.timestamp_millis();
            let query_path = self.query_dir.join(format!("{}.csv", timestamp_ms));
            let query_file = File::create(query_path)?;
            let mut query_writer = BufWriter::new(query_file);

            for page in &pages {
                let page_timestamp_ms = page.timestamp.timestamp_millis();
                write!(query_writer, "{},{}", page_timestamp_ms, page.url)?;

                let data_path = self.data_dir.join(if self.compression_level.is_none() {
                    format!("{}.json", page_timestamp_ms)
                } else {
                    format!("{}.json.zst", page_timestamp_ms)
                });
                let data_file = File::create(data_path)?;
                let mut data_writer: Box<dyn Write> = match self.compression_level {
                    Some(level) => {
                        Box::new(zstd::stream::Encoder::new(data_file, level)?.auto_finish())
                    }
                    None => Box::new(BufWriter::new(data_file)),
                };

                write!(data_writer, "{}", page.content)?;
            }

            Ok(pages.len())
        } else {
            Ok(0)
        }
    }
}

pub fn digests<P: AsRef<Path>>(
    base: P,
) -> Result<Box<dyn Iterator<Item = Result<String, Error>>>, std::io::Error> {
    let mut files = std::fs::read_dir(base)?.collect::<Result<Vec<_>, _>>()?;
    files.sort_by_key(|entry| entry.path());

    Ok(Box::new(files.into_iter().flat_map(|entry| {
        match File::open(entry.path())
            .and_then(zstd::stream::Decoder::new)
            .map_err(Error::from)
            .and_then(|decoder| {
                serde_json::from_reader::<_, Vec<Vec<String>>>(decoder)
                    .map_err(|error| Error::Json(error, entry.path().clone()))
            }) {
            Ok(items) => {
                let digests: Box<dyn Iterator<Item = Result<String, Error>>> =
                    Box::new(items.into_iter().map(|mut item| Ok(item.remove(5))));
                digests
            }
            Err(Error::Json(error, path)) => {
                cli_helpers::prelude::log::error!("{:?} at {:?}", error, path);
                Box::new(std::iter::empty())
            }
            Err(error) => Box::new(std::iter::once(Err(error))),
        }
    })))
}

pub fn redirect_digests<P: AsRef<Path>>(
    base: P,
) -> Result<Box<dyn Iterator<Item = Result<String, Error>>>, Error> {
    let mut files = std::fs::read_dir(base)?.collect::<Result<Vec<_>, _>>()?;
    files.sort_by_key(|entry| entry.path());

    Ok(Box::new(files.into_iter().flat_map(|entry| {
        match File::open(entry.path()).map_err(Error::from) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let digests: Box<dyn Iterator<Item = Result<String, Error>>> =
                    Box::new(reader.lines().map(|line| {
                        let line = line?;
                        Ok(line.split(',').next().unwrap().to_string())
                    }));
                digests
            }
            Err(error) => Box::new(std::iter::once(Err(error))),
        }
    })))
}
