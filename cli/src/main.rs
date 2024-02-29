use aib_indexer::{query::Range, Query};
use aib_manager::model::entry::InvalidDigest;
use aib_store::items::ValidationError;
use chrono::{NaiveDate, NaiveTime};
use cli_helpers::prelude::*;
use futures::stream::{StreamExt, TryStreamExt};
use sqlx::Connection;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let opts: Opts = Opts::parse();
    opts.verbose.init_logging()?;

    match opts.command {
        Command::Import {
            input,
            output,
            redirects,
        } => {
            let legacy_store = aib_store::legacy::wayback::Store::new(input);
            let new_store = aib_store::items::ItemStore::new(output, Some(14));
            let mut digests: HashSet<String> = new_store
                .entries(32)
                .filter_map(|result| async {
                    match result {
                        Ok(result) => result.ok().map(|entry| entry.digest.to_string()),
                        Err(error) => {
                            log::error!("{:?}", error);
                            None
                        }
                    }
                })
                .collect()
                .await;

            let redirect_digests = redirects
                .map(|redirects| {
                    aib_cdx_store::redirect_digests(redirects)
                        .and_then(|redirects| redirects.collect::<Result<HashSet<_>, _>>())
                })
                .transpose()?;

            digests.extend(redirect_digests.unwrap_or_default());

            /*for entry in legacy_store.paths() {
                let (digest, path) = entry?;

                if !digests.contains(&digest) {
                    let mut reader = GzDecoder::new(BufReader::new(File::open(path)?));
                    match new_store.save(&digest, &mut reader) {
                        Ok(written) => match written {
                            Some(bytes) => {
                                log::info!("Wrote {} bytes for {}", bytes, digest);
                            }
                            None => {
                                log::info!("Skipped {}", digest);
                            }
                        },
                        Err(error) => {
                            log::error!("{:?}", error)
                        }
                    }
                }
            }*/

            new_store
                .save_all::<Error, _>(
                    legacy_store
                        .paths()
                        .filter(|result| {
                            result
                                .as_ref()
                                .map(|(digest, _)| !digests.contains(digest))
                                .unwrap_or(true)
                        })
                        .map(|result| result.map_err(Error::from)),
                    16,
                )
                .for_each(|result| async {
                    match result {
                        Ok((digest, written)) => match written {
                            Some(bytes) => {
                                log::info!("Wrote {} bytes for {}", bytes, digest);
                            }
                            None => {
                                log::info!("Skipped {}", digest);
                            }
                        },
                        Err(error) => {
                            log::error!("{:?}", error)
                        }
                    }
                })
                .await;
        }
        Command::List { base } => {
            let new_store = aib_store::items::ItemStore::new(base, Some(14));
            for dir in new_store.files() {
                let dir = dir?;

                println!("{:?}", dir);
            }
        }
        Command::Validate { base } => {
            let new_store = aib_store::items::ItemStore::new(base, Some(14));

            new_store
                .entries(64)
                .try_for_each(|result| async move {
                    match result {
                        Ok(entry) => {
                            println!("{}", entry.digest);
                        }
                        Err(ValidationError::InvalidDigest { entry, digest: _ }) => {
                            log::error!("Expected {}", entry.digest);
                        }
                        Err(ValidationError::Unexpected(path)) => {
                            log::error!("Unexpected path: {:?}", path);
                        }
                    }
                    Ok(())
                })
                .await?;
        }
        Command::Invalid { base } => {
            let new_store = aib_store::items::ItemStore::new(base, Some(14));

            new_store
                .entries(64)
                .for_each(|result| async {
                    match result {
                        Ok(Err(ValidationError::InvalidDigest { entry, digest: _ })) => {
                            println!("{}", entry.path.as_os_str().to_string_lossy());
                        }
                        Err(aib_store::items::Error::ValidationIo { entry, error: _ }) => {
                            println!("{}", entry.path.as_os_str().to_string_lossy());
                        }
                        _ => {}
                    }
                })
                .await;
        }
        Command::Cdx {
            query,
            output,
            exact,
            start_page,
            level,
        } => {
            let client = aib_cdx::client::IndexClient::new_default()?;
            let cdx_store = Arc::new(aib_cdx_store::Store::new(&output, level));

            let (num_pages, pages) = client.lookup(&query, exact, start_page).await?;
            log::info!("Downloading {} pages for {}", num_pages, query);

            pages
                .map_err(Error::from)
                .try_for_each(|page| {
                    let cdx_store = cdx_store.clone();
                    async move {
                        cdx_store.add_entry_pages(&[aib_cdx_store::EntryPage::new(&page)])?;
                        Ok(())
                    }
                })
                .await?;
        }
        Command::CdxDump { base, level } => {
            let cdx_store = Arc::new(aib_cdx_store::Store::new(base, level));

            for (timestamp, entry) in cdx_store.entries()? {
                println!(
                    "{},{},{},{}",
                    timestamp.timestamp(),
                    entry.status_code.unwrap_or_default(),
                    entry.mime_type,
                    entry.original
                );
            }
        }
        Command::UnknownDigests {
            input,
            cdx,
            redirects,
        } => {
            let mut digests = BufReader::new(File::open(input)?)
                .lines()
                .collect::<Result<HashSet<_>, _>>()?;

            log::info!("Read {} digests", digests.len());

            let mut found = 0;
            let mut redirect_found = 0;
            let mut known = 0;

            for digest in aib_cdx_store::digests(cdx)? {
                let digest = digest?;

                if digests.remove(&digest) {
                    found += 1;
                }

                known += 1;
            }

            log::info!(
                "Found {} CDX entries in {} known (known is not distinct)",
                found,
                known
            );

            for digest in aib_cdx_store::redirect_digests(redirects)? {
                let digest = digest?;

                if digests.remove(&digest) {
                    redirect_found += 1;
                }
            }

            log::info!("Found {} redirect entries", redirect_found);
            log::info!("Missing: {}", digests.len());

            let mut digests = digests.into_iter().collect::<Vec<_>>();
            digests.sort();

            for digest in digests {
                println!("{}", digest);
            }
        }
        Command::ManagerExtract {
            index,
            item_store,
            item_level,
        } => {
            let manager = aib_manager::Manager::open(
                "sqlite://manager/data/state.db",
                index,
                item_store,
                item_level,
            )
            .await?;

            manager.extract()?;
        }
        Command::ManagerIndex {
            index,
            item_store,
            item_level,
        } => {
            let mut manager = aib_manager::Manager::open(
                "sqlite://manager/data/state.db",
                index,
                item_store,
                item_level,
            )
            .await?;

            let count = manager.index("text/html").await?;

            log::info!("Indexed {} documents", count);
        }
        Command::Search {
            index,
            item_store,
            item_level,
            query,
            email,
            start_date,
            end_date,
            pattern,
            year,
            limit,
            offset,
        } => {
            let mut manager = aib_manager::Manager::open(
                "sqlite://manager/data/state.db",
                index,
                item_store,
                item_level,
            )
            .await?;

            log::info!(
                "Initialized {} SURT IDs",
                manager.index.initialize_surt_ids()?
            );

            let date_range = Range::new(start_date, end_date);
            let date_time_range =
                date_range.map(|range| range.map(|value| value.and_time(NaiveTime::MIN).and_utc()));

            let query = Query::new(
                &query,
                email.as_deref(),
                date_time_range,
                pattern.unwrap_or_default(),
                year.unwrap_or_default(),
            );

            let result = manager.search(100, &query, limit, offset).await?;

            for (surt, surt_results) in result.surts {
                println!("{:?}", surt);
                for (timestamp, result) in surt_results {
                    println!(
                        "    {}: {}",
                        timestamp,
                        result
                            .map(|value| serde_json::json!(value))
                            .unwrap_or_default()
                    );
                }
            }
        }
        Command::CdxImport { config, db_url } => {
            let mut connection = sqlx::SqliteConnection::connect(&db_url).await?;

            aib_manager::import::run_import(&config, &mut connection).await?;
        }
        Command::LocalSnapshotImport {
            db_url,
            store,
            level,
            mime_type,
        } => {
            let store = aib_store::items::ItemStore::new(store, level);
            let mut connection = sqlx::SqliteConnection::connect(&db_url).await?;

            let count =
                aib_manager::import::find_local_snapshots(&mut connection, &store, &mime_type)
                    .await?;

            log::info!("Added {} snapshots", count);
        }
        Command::MissingSnapshots { db_url, mime_type } => {
            let mut connection = sqlx::SqliteConnection::connect(&db_url).await?;

            let mut writer = csv::WriterBuilder::new()
                .has_headers(false)
                .from_writer(std::io::stdout());

            for entry in
                aib_manager::import::list_missing_snapshots(&mut connection, &mime_type).await?
            {
                writer.serialize(entry)?;
            }
        }

        Command::InvalidDigests { db_url } => {
            let mut connection = sqlx::SqliteConnection::connect(&db_url).await?;

            let invalid_digests =
                aib_manager::import::list_invalid_digests(&mut connection).await?;

            let mut writer = csv::WriterBuilder::new()
                .has_headers(false)
                .from_writer(std::io::stdout());

            for invalid_digest in invalid_digests {
                writer.serialize(&invalid_digest)?;
            }
        }
        Command::ImportInvalidDigests {
            db_url,
            store,
            level,
        } => {
            let store = aib_store::items::ItemStore::new(store, level);
            let mut connection = sqlx::SqliteConnection::connect(&db_url).await?;

            let invalid_digests = csv::ReaderBuilder::new()
                .has_headers(false)
                .from_reader(std::io::stdin())
                .deserialize::<InvalidDigest>()
                .collect::<Result<Vec<_>, _>>()?;

            let count = aib_manager::import::import_invalid_digests(
                &mut connection,
                &store,
                &invalid_digests,
            )
            .await?;

            log::info!("Added {} entry successes", count);
        }
    }

    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("CLI argument reading error")]
    Args(#[from] cli_helpers::Error),
    #[error("CSV error")]
    Csv(#[from] csv::Error),
    #[error("Legacy store error")]
    LegacyStore(#[from] aib_store::legacy::wayback::Error),
    #[error("Store error")]
    Store(#[from] aib_store::Error),
    #[error("Item store error")]
    ItemStore(#[from] aib_store::items::Error),
    #[error("CDX index error")]
    Cdx(#[from] aib_cdx::client::Error),
    #[error("CDX store error")]
    CdxStore(#[from] aib_cdx_store::Error),
    #[error("Manager error")]
    Manager(#[from] aib_manager::Error),
    #[error("Manager import error")]
    ManagerImport(#[from] aib_manager::import::Error),
    #[error("Index error")]
    Index(#[from] aib_indexer::Error),
    #[error("SQLx error")]
    Sqlx(#[from] sqlx::Error),
}

#[derive(Debug, Parser)]
#[clap(name = "wb", version, author)]
struct Opts {
    #[clap(flatten)]
    verbose: Verbosity,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Parser)]
enum Command {
    Import {
        #[clap(long)]
        input: PathBuf,
        #[clap(long)]
        output: PathBuf,
        #[clap(long)]
        redirects: Option<PathBuf>,
    },
    List {
        #[clap(long)]
        base: PathBuf,
    },
    Validate {
        #[clap(long)]
        base: PathBuf,
    },
    Invalid {
        #[clap(long)]
        base: PathBuf,
    },
    Cdx {
        #[clap(long)]
        query: String,
        #[clap(long)]
        output: PathBuf,
        #[clap(long)]
        exact: bool,
        #[clap(long)]
        start_page: Option<usize>,
        #[clap(long)]
        level: Option<i32>,
    },
    CdxDump {
        #[clap(long)]
        base: PathBuf,
        #[clap(long)]
        level: Option<i32>,
    },
    UnknownDigests {
        #[clap(long)]
        input: PathBuf,
        #[clap(long)]
        cdx: PathBuf,
        #[clap(long)]
        redirects: PathBuf,
    },
    ManagerExtract {
        #[clap(long)]
        index: PathBuf,
        #[clap(long)]
        item_store: PathBuf,
        #[clap(long)]
        item_level: Option<i32>,
    },
    ManagerIndex {
        #[clap(long)]
        index: PathBuf,
        #[clap(long)]
        item_store: PathBuf,
        #[clap(long)]
        item_level: Option<i32>,
    },
    Search {
        #[clap(long)]
        index: PathBuf,
        #[clap(long)]
        item_store: PathBuf,
        #[clap(long)]
        item_level: Option<i32>,
        #[clap(long)]
        query: String,
        #[clap(long)]
        email: Option<String>,
        #[clap(long)]
        start_date: Option<NaiveDate>,
        #[clap(long)]
        end_date: Option<NaiveDate>,
        #[clap(long)]
        pattern: Option<Vec<String>>,
        #[clap(long)]
        year: Option<Vec<u16>>,
        #[clap(long, default_value = "100")]
        limit: usize,
        #[clap(long, default_value = "0")]
        offset: usize,
    },
    CdxImport {
        #[clap(long)]
        config: PathBuf,
        #[clap(long)]
        db_url: String,
    },
    LocalSnapshotImport {
        #[clap(long)]
        db_url: String,
        #[clap(long)]
        store: PathBuf,
        #[clap(long)]
        level: Option<i32>,
        #[clap(long, default_value = "text/html")]
        mime_type: String,
    },
    MissingSnapshots {
        #[clap(long)]
        db_url: String,
        #[clap(long, default_value = "text/html")]
        mime_type: String,
    },
    InvalidDigests {
        #[clap(long)]
        db_url: String,
    },
    ImportInvalidDigests {
        #[clap(long)]
        db_url: String,
        #[clap(long)]
        store: PathBuf,
        #[clap(long)]
        level: Option<i32>,
    },
}
