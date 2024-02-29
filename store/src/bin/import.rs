use cli_helpers::prelude::*;
use futures::stream::TryStreamExt;
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
}
