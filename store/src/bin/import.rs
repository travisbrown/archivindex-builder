use aib_core::digest::Sha1Digest;
use cli_helpers::prelude::*;
use futures::stream::TryStreamExt;
use parquetry::Schema;
use std::fs::File;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let opts: Opts = Opts::parse();
    opts.verbose.init_logging()?;

    match opts.command {
        Command::Import {
            input,
            output,
            level,
        } => {
            let store = aib_store::items::ItemStore::new(output, level);
            let files = std::fs::read_dir(input)?
                .map(|entry| {
                    let entry = entry?;
                    let file_name = entry
                        .file_name()
                        .into_string()
                        .ok()
                        .ok_or_else(|| aib_store::items::Error::Unexpected(entry.path()))?;

                    Ok((file_name, entry.path()))
                })
                .collect::<Result<Vec<_>, Error>>()?;

            for (digest, path) in files {
                let mut file = File::open(path)?;
                store.save(&digest, &mut file)?;
            }
        }
        Command::ImportLegacy {
            input,
            output,
            level,
        } => {
            let store = aib_store::items::ItemStore::new(output, level);
            for result in aib_store::legacy::import_gz(input)? {
                let (file_stem, mut reader) = result?;

                store.save(&file_stem, &mut reader)?;
            }
        }
        Command::Validate { input, level } => {
            let store = aib_store::items::ItemStore::new(input, level);
            store
                .entries(4)
                .try_for_each(|entry| async {
                    match entry {
                        Ok(_entry) => {}
                        Err(error) => {
                            log::error!("{:?}", error);
                        }
                    }

                    Ok(())
                })
                .await?;
        }
        Command::List { input, level } => {
            let store = aib_store::items::ItemStore::new(input, level);
            store
                .entries(4)
                .try_for_each(|entry| async {
                    match entry {
                        Ok(entry) => {
                            println!("{}", entry.digest);
                        }
                        Err(error) => {
                            log::error!("{:?}", error);
                        }
                    }

                    Ok(())
                })
                .await?;
        }
        Command::Parquetify {
            input,
            output,
            level,
        } => {
            use aib_store::item::{columns, Item};
            use parquet::file::properties::WriterProperties;
            use parquetry::Schema;
            let store = aib_store::items::ItemStore::new(input, level);
            let mut file = std::fs::File::create(&output)?;
            let properties = WriterProperties::builder()
                .set_writer_version(parquet::file::properties::WriterVersion::PARQUET_2_0)
                .set_sorting_columns(Some(vec![columns::DIGEST.sorting()]))
                .set_column_dictionary_enabled(columns::DIGEST.path(), false)
                .set_column_dictionary_enabled(columns::CONTENT.path(), false)
                .set_column_encoding(
                    columns::DIGEST.path(),
                    parquet::basic::Encoding::DELTA_BYTE_ARRAY,
                )
                .set_column_bloom_filter_enabled(columns::DIGEST.path(), true)
                .set_column_encoding(
                    columns::CONTENT.path(),
                    parquet::basic::Encoding::DELTA_LENGTH_BYTE_ARRAY,
                )
                .set_column_compression(
                    columns::CONTENT.path(),
                    parquet::basic::Compression::ZSTD(parquet::basic::ZstdLevel::try_new(
                        level.unwrap_or_default(),
                    )?),
                )
                .set_column_statistics_enabled(
                    columns::CONTENT.path(),
                    parquet::file::properties::EnabledStatistics::None,
                )
                .build();

            let mut files = store
                .entries(4)
                .and_then(|entry| async {
                    let entry = entry.unwrap();
                    //let bytes = zstd::stream::decode_all(File::open(entry.path)?)?;
                    //let item = Item::new(entry.digest.0, bytes).unwrap();
                    Ok(entry)
                })
                .try_collect::<Vec<_>>()
                .await?;

            files.sort_by_key(|entry| entry.digest);

            let groups = files.chunks(10000).map(|entries| {
                entries
                    .iter()
                    .map(|entry| {
                        let bytes =
                            zstd::stream::decode_all(File::open(&entry.path).unwrap()).unwrap();
                        let item = Item::new(entry.digest.0, bytes).unwrap();
                        item
                    })
                    .collect::<Vec<_>>()
            });

            let data = Item::write(file, properties, groups)?;

            println!("{:?}", data);
        }
        Command::ParquetDump { input } => {
            /*use aib_store::item::Item;
            use parquet::file::serialized_reader::ReadOptionsBuilder;

            for item in Item::read(File::open(input)?, ReadOptionsBuilder::new().build()) {
                let item = item?;
                let digest = Sha1Digest(item.digest);
                let content = std::str::from_utf8(&item.content).unwrap();

                println!("{}: {}", digest, &content[0..10]);
            }*/

            aib_store::parquet::read_parquet(File::open(input)?).unwrap();
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
    #[error("Legacy store error")]
    LegacyStore(#[from] aib_store::legacy::wayback::Error),
    #[error("Store error")]
    Store(#[from] aib_store::Error),
    #[error("Item store error")]
    ItemStore(#[from] aib_store::items::Error),
    #[error("Parquet error")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("Parquetry error")]
    Parquetry(#[from] parquetry::error::Error),
}

#[derive(Debug, Parser)]
#[clap(name = "wb-store-import", version, author)]
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
        level: Option<i32>,
    },
    ImportLegacy {
        #[clap(long)]
        input: PathBuf,
        #[clap(long)]
        output: PathBuf,
        #[clap(long)]
        level: Option<i32>,
    },
    Validate {
        #[clap(long)]
        input: PathBuf,
        #[clap(long)]
        level: Option<i32>,
    },
    List {
        #[clap(long)]
        input: PathBuf,
        #[clap(long)]
        level: Option<i32>,
    },
    Parquetify {
        #[clap(long)]
        input: PathBuf,
        #[clap(long)]
        output: PathBuf,
        #[clap(long)]
        level: Option<i32>,
    },
    ParquetDump {
        #[clap(long)]
        input: PathBuf,
    },
}
