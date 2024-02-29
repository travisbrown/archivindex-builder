use aib_core::{
    digest::{Digest, Sha1Computer, Sha1Digest},
    entry::{EntryInfo, UrlParts},
    timestamp::Timestamp,
};
use cli_helpers::prelude::*;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::PathBuf;

/// Invalid digest for a CDX entry.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
pub struct InvalidDigest {
    pub url: String,
    pub timestamp: Timestamp,
    pub expected: Digest,
    pub actual: Sha1Digest,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let opts: Opts = Opts::parse();
    opts.verbose.init_logging()?;

    let entries = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(std::io::stdin())
        .deserialize::<EntryInfo>()
        .collect::<Result<Vec<_>, _>>()?;

    let output_data_dir = opts.output.join("data");
    let output_invalid_digests_file = opts.output.join("invalid-digests.csv");

    std::fs::create_dir_all(&output_data_dir)?;

    let mut invalid_digests = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(File::create(output_invalid_digests_file)?);

    let downloader = aib_downloader::Downloader::default();
    let sha1_computer = Sha1Computer::default();

    for EntryInfo {
        url_parts: UrlParts { url, timestamp },
        expected_digest,
    } in entries
    {
        log::info!("Downloading {} ({})", url, timestamp);

        if let Some(result) = downloader.download(&url, timestamp, true).await? {
            for redirect in result.redirects {
                log::warn!("Redirecting: {} ({}) to {}", url, timestamp, redirect.url);
            }

            let digest = sha1_computer.digest(&mut Cursor::new(&result.bytes))?;

            if Digest::Valid(digest) != expected_digest {
                log::warn!("Invalid digest: {} instead of {}", digest, expected_digest);

                invalid_digests.serialize(InvalidDigest {
                    url: url.clone(),
                    timestamp,
                    expected: expected_digest,
                    actual: digest,
                })?;
                invalid_digests.flush()?;
            }

            log::info!("Saving {}", digest);

            let mut file = File::create(output_data_dir.join(digest.to_string()))?;
            file.write_all(&result.bytes)?;
        } else {
            log::warn!("Skipped: {} ({})", url, timestamp);
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
    #[error("Downloader error")]
    Downloader(#[from] aib_downloader::Error),
}

#[derive(Debug, Parser)]
#[clap(name = "wb-downloader-cli", version, author)]
struct Opts {
    #[clap(flatten)]
    verbose: Verbosity,
    #[clap(long)]
    output: PathBuf,
}
