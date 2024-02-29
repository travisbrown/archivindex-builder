use aib_core::digest::{compute_digest, Sha1Digest};
use flate2::bufread::GzDecoder;
use futures::{FutureExt, Stream, TryStreamExt};
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use zstd::Decoder;

const DEFAULT_COMPRESSION_LEVEL: i32 = 14;

pub mod iter;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub digest: Sha1Digest,
}

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error("Unexpected path")]
    Unexpected(PathBuf),
    #[error("Invalid digest")]
    InvalidDigest { entry: Entry, digest: String },
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unexpected file")]
    Unexpected(PathBuf),
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("Invalid digest")]
    InvalidDigest(String),
    #[error("Digest error")]
    Digest(#[from] aib_core::digest::Error),
    #[error("Task error")]
    Task(#[from] tokio::task::JoinError),
    #[error("Import I/O error")]
    ImportIo {
        digest: String,
        error: std::io::Error,
    },
    #[error("Validation I/O error")]
    ValidationIo { entry: Entry, error: std::io::Error },
}

lazy_static! {
    static ref NAMES: HashSet<String> = {
        let mut names = HashSet::new();
        names.extend(('2'..='7').map(|c| c.to_string()));
        names.extend(('A'..='Z').map(|c| c.to_string()));
        names
    };
}

fn is_valid_char(c: char) -> bool {
    ('2'..='7').contains(&c) || c.is_ascii_uppercase()
}

fn validate_directory_path(path: &Path) -> bool {
    if path.is_dir() {
        match path.file_name().and_then(|filename| filename.to_str()) {
            Some(filename) => filename.chars().count() == 2 && filename.chars().all(is_valid_char),
            None => false,
        }
    } else {
        false
    }
}

fn validate(path: &Path) -> Result<Entry, ValidationError> {
    let level1 = path
        .parent()
        .ok_or_else(|| ValidationError::Unexpected(path.to_path_buf()))?;
    let level0 = level1
        .parent()
        .ok_or_else(|| ValidationError::Unexpected(path.to_path_buf()))?;

    if validate_directory_path(level0) && validate_directory_path(level1) {
        let filename = path
            .file_name()
            .and_then(|filename| filename.to_str())
            .ok_or_else(|| ValidationError::Unexpected(path.to_path_buf()))?;

        if filename.chars().count() == 36
            && filename.chars().take(32).all(is_valid_char)
            && filename.ends_with(".zst")
        {
            // Safe because we've just validated the filename.
            let digest = filename[0..32].to_string().parse().unwrap();

            Ok(Entry {
                path: path.to_path_buf(),
                digest,
            })
        } else {
            Err(ValidationError::Unexpected(path.to_path_buf()))
        }
    } else {
        Err(ValidationError::Unexpected(path.to_path_buf()))
    }
}

/// A content-addressable store for compressed Wayback Machine pages.
#[derive(Clone, Debug)]
pub struct ItemStore {
    base: PathBuf,
    compression_level: i32,
}

impl ItemStore {
    pub fn new<P: AsRef<Path>>(path: P, compression_level: Option<i32>) -> Self {
        Self {
            base: path.as_ref().to_path_buf(),
            compression_level: compression_level.unwrap_or(DEFAULT_COMPRESSION_LEVEL),
        }
    }

    fn is_valid_digest(candidate: &str) -> bool {
        candidate.len() == 32 && candidate.chars().all(is_valid_char)
    }

    pub fn location(&self, digest: &str) -> Option<PathBuf> {
        if Self::is_valid_digest(digest) {
            let bytes = digest.as_bytes();
            // Safe because we've just validated the digest.
            let p0 = std::str::from_utf8(&bytes[0..2]).unwrap();
            let p1 = std::str::from_utf8(&bytes[2..4]).unwrap();

            Some(self.base.join(p0).join(p1).join(format!("{}.zst", digest)))
        } else {
            None
        }
    }

    pub fn contains(&self, digest: &str) -> bool {
        self.location(digest)
            .map(|path| path.is_file())
            .unwrap_or(false)
    }

    pub fn save_all<'a, E: 'a, I: 'a + Iterator<Item = Result<(String, PathBuf), E>>>(
        &'a self,
        items: I,
        parallelism: usize,
    ) -> impl Stream<Item = Result<(String, Option<u64>), E>> + '_
    where
        E: From<Error>,
    {
        futures::stream::iter(items)
            .map_ok(|(digest, path)| {
                let store = self.clone();
                tokio::spawn(async move {
                    let mut reader = GzDecoder::new(BufReader::new(File::open(path)?));
                    store
                        .save(&digest, &mut reader)
                        .map(|value| (digest, value))
                })
                .map(|result| match result {
                    Ok(Ok(value)) => Ok(value),
                    Ok(Err(error)) => Err(error.into()),
                    Err(error) => Err(Error::from(error).into()),
                })
            })
            .try_buffer_unordered(parallelism)
    }

    pub fn save<R: Read>(&self, digest: &str, reader: &mut R) -> Result<Option<u64>, Error> {
        let path = self
            .location(digest)
            .ok_or_else(|| Error::InvalidDigest(digest.to_string()))?;

        if path.exists() {
            Ok(None)
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut writer =
                zstd::stream::write::Encoder::new(File::create(path)?, self.compression_level)?;

            let written = std::io::copy(reader, &mut writer).map_err(|error| Error::ImportIo {
                digest: digest.to_string(),
                error,
            })?;

            writer.finish()?;

            Ok(Some(written))
        }
    }

    pub fn directories(&self) -> iter::DirectoryIter {
        iter::DirectoryIter::new(&self.base)
    }

    pub fn files(&self) -> iter::FileIter {
        iter::FileIter::new(self.directories())
    }

    pub fn entries(
        &self,
        parallelism: usize,
    ) -> impl Stream<Item = Result<Result<Entry, ValidationError>, Error>> {
        futures::stream::iter(self.files())
            .map_err(Error::from)
            .map_ok(|path| {
                tokio::spawn(async move {
                    match validate(&path) {
                        Ok(entry) => {
                            let file_digest = File::open(&path)
                                .and_then(Decoder::new)
                                .and_then(|mut reader| compute_digest(&mut reader))
                                .map_err(|error| Error::ValidationIo {
                                    entry: entry.clone(),
                                    error,
                                })?;

                            if file_digest == entry.digest {
                                Ok(Ok(entry))
                            } else {
                                Ok(Err(ValidationError::InvalidDigest {
                                    entry,
                                    digest: file_digest.to_string(),
                                }))
                            }
                        }
                        Err(error) => Ok(Err(error)),
                    }
                })
                .map(|result| match result {
                    Ok(Ok(value)) => Ok(value),
                    Ok(Err(error)) => Err(error),
                    Err(error) => Err(error.into()),
                })
            })
            .try_buffer_unordered(parallelism)
    }

    /*pub fn validate(&self) -> Result<(usize, Vec<String>), Error> {
        for ()
    }

    pub fn paths(&self) -> impl Iterator<Item = Result<(String, PathBuf), Error>> {
        match read_dir(&self.base).and_then(|entries| entries.collect::<Result<Vec<_>, _>>()) {
            Err(error) => Self::emit_error(error),
            Ok(mut dirs) => {
                dirs.sort_by_key(|entry| entry.file_name());
                Box::new(
                    dirs.into_iter()
                        .flat_map(|entry| match Self::check_dir_entry(&entry) {
                            Err(error) => Self::emit_error(error),
                            Ok(first) => match read_dir(entry.path()) {
                                Err(error) => Self::emit_error(error),
                                Ok(files) => Box::new(files.map(move |result| {
                                    result
                                        .map_err(Error::from)
                                        .and_then(|entry| Self::check_file_entry(&first, &entry))
                                })),
                            },
                        }),
                )
            }
        }
    }*/

    /*pub fn create<P: AsRef<Path>>(base: P) -> Result<Self, std::io::Error> {
        let path = base.as_ref();

        for name in NAMES.iter() {
            std::fs::create_dir_all(path.join(name))?;
        }

        Ok(Store {
            base: path.to_path_buf(),
        })
    }*/

    /*pub fn compute_digests(
        &self,
        prefix: Option<&str>,
        n: usize,
    ) -> impl Stream<Item = Result<(String, String), Error>> {
        futures::stream::iter(self.paths_for_prefix(prefix.unwrap_or("")))
            .map_ok(|(expected, path)| {
                tokio::spawn(async {
                    let mut file = File::open(path)?;
                    match compute_digest(&mut GzDecoder::new(&mut file)) {
                        Ok(actual) => Ok((expected, actual)),
                        Err(error) => Err(Error::from(error)),
                    }
                })
                .map(|result| match result {
                    Ok(Err(error)) => Err(error),
                    Ok(Ok(value)) => Ok(value),
                    Err(_) => Err(Error::DigestComputationError),
                })
            })
            .try_buffer_unordered(n)
    }

    fn emit_error<T: 'static, E: Into<Error>>(e: E) -> Box<dyn Iterator<Item = Result<T, Error>>> {
        Box::new(once(Err(e.into())))
    }

    pub fn paths(&self) -> impl Iterator<Item = Result<(String, PathBuf), Error>> {
        match read_dir(&self.base).and_then(|it| it.collect::<std::result::Result<Vec<_>, _>>()) {
            Err(error) => Self::emit_error(error),
            Ok(mut dirs) => {
                dirs.sort_by_key(|entry| entry.file_name());
                Box::new(
                    dirs.into_iter()
                        .flat_map(|entry| match Self::check_dir_entry(&entry) {
                            Err(error) => Self::emit_error(error),
                            Ok(first) => match read_dir(entry.path()) {
                                Err(error) => Self::emit_error(error),
                                Ok(files) => Box::new(files.map(move |result| {
                                    result
                                        .map_err(Error::from)
                                        .and_then(|entry| Self::check_file_entry(&first, &entry))
                                })),
                            },
                        }),
                )
            }
        }
    }

    pub fn paths_for_prefix(
        &self,
        prefix: &str,
    ) -> impl Iterator<Item = Result<(String, PathBuf), Error>> {
        match prefix.chars().next() {
            None => Box::new(self.paths()),
            Some(first_char) => {
                if Self::is_valid_prefix(prefix) {
                    let first = first_char.to_string();
                    match read_dir(self.base.join(&first)) {
                        Err(error) => Self::emit_error(error),
                        Ok(files) => {
                            let p = prefix.to_string();
                            Box::new(
                                files
                                    .map(move |result| {
                                        result.map_err(Error::from).and_then(|entry| {
                                            Self::check_file_entry(&first, &entry)
                                        })
                                    })
                                    .filter(move |result| match result {
                                        Ok((name, _)) => name.starts_with(&p),
                                        Err(_) => true,
                                    }),
                            )
                        }
                    }
                } else {
                    Self::emit_error(Error::InvalidDigest(prefix.to_string()))
                }
            }
        }
    }

    pub fn check_file_location<P: AsRef<Path>>(
        &self,
        candidate: P,
    ) -> Result<Option<(String, Result<Box<Path>, String>)>, Error> {
        let path = candidate.as_ref();

        if let Some((name, ext)) = path
            .file_stem()
            .and_then(|os| os.to_str())
            .zip(path.extension().and_then(|os| os.to_str()))
        {
            if Self::is_valid_digest(name) && ext == "gz" {
                if let Some(location) = self.location(name) {
                    if location.is_file() {
                        Ok(None)
                    } else {
                        let mut file = File::open(path)?;
                        let digest = compute_digest(&mut GzDecoder::new(&mut file))?;

                        Ok(Some((
                            name.to_string(),
                            if digest == name {
                                Ok(location)
                            } else {
                                Err(digest)
                            },
                        )))
                    }
                } else {
                    Err(Error::InvalidDigest(name.to_string()))
                }
            } else {
                Err(Error::InvalidDigest(name.to_string()))
            }
        } else {
            Err(Error::InvalidDigest(path.to_string_lossy().into_owned()))
        }
    }

    pub fn location(&self, digest: &str) -> Option<Box<Path>> {
        if Self::is_valid_digest(digest) {
            digest.chars().next().map(|first_char| {
                let path = self
                    .base
                    .join(first_char.to_string())
                    .join(format!("{}.gz", digest));

                path.into_boxed_path()
            })
        } else {
            None
        }
    }

    pub fn contains(&self, digest: &str) -> bool {
        self.lookup(digest).is_some()
    }

    pub fn lookup(&self, digest: &str) -> Option<Box<Path>> {
        self.location(digest).filter(|path| path.is_file())
    }

    pub fn extract_reader(
        &self,
        digest: &str,
    ) -> Option<Result<BufReader<GzDecoder<File>>, std::io::Error>> {
        self.lookup(digest).map(|path| {
            let file = File::open(path)?;

            Ok(BufReader::new(GzDecoder::new(file)))
        })
    }

    pub fn extract(&self, digest: &str) -> Option<Result<String, std::io::Error>> {
        self.lookup(digest).map(|path| {
            let file = File::open(path)?;
            let mut buffer = String::new();

            GzDecoder::new(file).read_to_string(&mut buffer)?;

            Ok(buffer)
        })
    }

    pub fn extract_bytes(&self, digest: &str) -> Option<Result<Vec<u8>, std::io::Error>> {
        self.lookup(digest).map(|path| {
            let file = File::open(path)?;
            let mut buffer = Vec::new();

            GzDecoder::new(file).read_to_end(&mut buffer)?;

            Ok(buffer)
        })
    }

    fn is_valid_digest(candidate: &str) -> bool {
        candidate.len() == 32 && candidate.chars().all(is_valid_char)
    }

    fn is_valid_prefix(candidate: &str) -> bool {
        candidate.len() <= 32 && candidate.chars().all(is_valid_char)
    }*/

    /*fn check_file_entry(first: &str, entry: &DirEntry) -> Result<(String, PathBuf), Error> {
        if entry.file_type()?.is_file() {
            let path = entry.path();
            match entry.path().file_stem().and_then(|os| os.to_str()) {
                None => Err(Error::Unexpected(entry.path())
                Some(name) => {
                    if name.starts_with(first) {
                        Ok((name.to_string(), entry.path()))
                    } else {
                        Err(Error::Unexpected {
                            path: entry.path().into_boxed_path(),
                        })
                    }
                }
            }
        } else {
            Err(Error::Unexpected {
                path: entry.path().into_boxed_path(),
            })
        }
    }

    fn check_dir_entry(entry: &DirEntry) -> Result<String, Error> {
        if entry.file_type()?.is_dir() {
            match entry.file_name().into_string() {
                Err(_) => Err(Error::Unexpected {
                    path: entry.path().into_boxed_path(),
                }),
                Ok(name) => {
                    if NAMES.contains(&name) {
                        Ok(name)
                    } else {
                        Err(Error::Unexpected {
                            path: entry.path().into_boxed_path(),
                        })
                    }
                }
            }
        } else {
            Err(Error::Unexpected {
                path: entry.path().into_boxed_path(),
            })
        }
    }*/
}
